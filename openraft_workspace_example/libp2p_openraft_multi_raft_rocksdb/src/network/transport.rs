use std::{collections::HashMap, future::Future, sync::Arc};

use anyhow::Context;
use libp2p::{Multiaddr, PeerId, multiaddr::Protocol};
use openraft::{
  BasicNode, RaftNetworkFactory,
  network::{RPCOption, v2::RaftNetworkV2},
};

use crate::{
  GroupId, NodeId, Unreachable,
  network::{
    rpc::{RaftRpcOp, RaftRpcRequest, RaftRpcResponse},
    swarm::{Libp2pClient, NetErr},
  },
  typ::{
    AppendEntriesRequest, AppendEntriesResponse, RPCError, Snapshot, SnapshotResponse,
    StreamingError, Vote, VoteRequest, VoteResponse,
  },
};

#[derive(Clone)]
pub struct Libp2pNetworkFactory {
  client: Libp2pClient,
  node_peers: Arc<tokio::sync::RwLock<HashMap<NodeId, (PeerId, Multiaddr)>>>,
  group_id: Option<GroupId>,
}

impl Libp2pNetworkFactory {
  pub fn new(client: Libp2pClient) -> Self {
    Self {
      client,
      node_peers: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
      group_id: None,
    }
  }

  pub fn with_group(&self, group_id: GroupId) -> Self {
    Self {
      client: self.client.clone(),
      node_peers: self.node_peers.clone(),
      group_id: Some(group_id),
    }
  }

  pub async fn register_node(&self, node_id: NodeId, addr: &str) -> anyhow::Result<()> {
    let (peer, maddr) = parse_p2p_addr(addr)?;
    {
      let mut map = self.node_peers.write().await;
      map.insert(node_id, (peer, maddr.clone()));
    }
    self.client.dial(maddr).await;
    Ok(())
  }

  pub async fn update_peer_addr_from_mdns(&self, peer: PeerId, addr: Multiaddr) {
    let mut map = self.node_peers.write().await;
    for (node_id, (stored_peer, stored_addr)) in map.iter_mut() {
      if *stored_peer != peer {
        continue;
      }

      let candidate = ensure_p2p_addr(addr.clone(), peer);
      if candidate == *stored_addr {
        return;
      }

      let candidate_loopback = is_loopback_addr(&candidate);
      let stored_loopback = is_loopback_addr(stored_addr);
      if candidate_loopback != stored_loopback {
        return;
      }

      tracing::info!(
        node_id = *node_id,
        peer = %peer,
        addr = %candidate,
        "updating peer address from mdns"
      );
      *stored_addr = candidate;
      return;
    }
  }

  pub async fn known_nodes(&self) -> Vec<(NodeId, PeerId, Multiaddr)> {
    let map = self.node_peers.read().await;
    map
      .iter()
      .map(|(id, (peer, addr))| (*id, *peer, addr.clone()))
      .collect()
  }

  pub async fn publish_gossipsub(&self, topic: &str, data: Vec<u8>) -> Result<(), NetErr> {
    self.client.publish_gossipsub(topic, data).await
  }

  pub async fn request(
    &self,
    node_id: NodeId,
    req: RaftRpcRequest,
  ) -> Result<RaftRpcResponse, Unreachable> {
    let (peer, addr) = self.peer_addr_for(node_id).await?;
    self.client.dial(addr).await;
    self.client.request(peer, req).await
  }

  async fn peer_addr_for(&self, node_id: NodeId) -> Result<(PeerId, Multiaddr), Unreachable> {
    let map = self.node_peers.read().await;
    map
      .get(&node_id)
      .map(|(peer, addr)| (*peer, addr.clone()))
      .ok_or_else(|| Unreachable::new(&NetErr(format!("unknown target node_id={node_id}"))))
  }

  async fn peer_for(&self, node_id: NodeId) -> Result<PeerId, Unreachable> {
    let map = self.node_peers.read().await;
    map
      .get(&node_id)
      .map(|(peer, _)| *peer)
      .ok_or_else(|| Unreachable::new(&NetErr(format!("unknown target node_id={node_id}"))))
  }
}

pub struct Libp2pConnection {
  target: NodeId,
  factory: Libp2pNetworkFactory,
  group_id: GroupId,
}

impl RaftNetworkFactory<openraft_rocksstore_crud::TypeConfig> for Libp2pNetworkFactory {
  type Network = Libp2pConnection;

  async fn new_client(&mut self, target: NodeId, node: &BasicNode) -> Self::Network {
    let _ = self.register_node(target, &node.addr).await;
    let group_id = self
      .group_id
      .clone()
      .expect("group_id required for raft network");

    Libp2pConnection {
      target,
      factory: self.clone(),
      group_id,
    }
  }
}

impl RaftNetworkV2<openraft_rocksstore_crud::TypeConfig> for Libp2pConnection {
  async fn append_entries(
    &mut self,
    req: AppendEntriesRequest,
    _option: RPCOption,
  ) -> Result<AppendEntriesResponse, RPCError> {
    let peer = self.factory.peer_for(self.target).await?;
    let resp = self
      .factory
      .client
      .request(
        peer,
        RaftRpcRequest {
          group_id: self.group_id.clone(),
          op: RaftRpcOp::AppendEntries(req),
        },
      )
      .await?;

    match resp {
      RaftRpcResponse::AppendEntries(r) => {
        r.map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))
      }
      other => Err(RPCError::Unreachable(Unreachable::new(&NetErr(format!(
        "unexpected response: {other:?}"
      ))))),
    }
  }

  async fn vote(&mut self, req: VoteRequest, _option: RPCOption) -> Result<VoteResponse, RPCError> {
    let peer = self.factory.peer_for(self.target).await?;
    let resp = self
      .factory
      .client
      .request(
        peer,
        RaftRpcRequest {
          group_id: self.group_id.clone(),
          op: RaftRpcOp::Vote(req),
        },
      )
      .await?;

    match resp {
      RaftRpcResponse::Vote(r) => r.map_err(|e| RPCError::Unreachable(Unreachable::new(&e))),
      other => Err(RPCError::Unreachable(Unreachable::new(&NetErr(format!(
        "unexpected response: {other:?}"
      ))))),
    }
  }

  async fn full_snapshot(
    &mut self,
    vote: Vote,
    snapshot: Snapshot,
    _cancel: impl Future<Output = openraft::error::ReplicationClosed> + openraft::OptionalSend + 'static,
    _option: RPCOption,
  ) -> Result<SnapshotResponse, StreamingError> {
    let peer = self
      .factory
      .peer_for(self.target)
      .await
      .map_err(StreamingError::Unreachable)?;

    let data: Vec<u8> = snapshot.snapshot.into_inner();

    let resp = self
      .factory
      .client
      .request(
        peer,
        RaftRpcRequest {
          group_id: self.group_id.clone(),
          op: RaftRpcOp::FullSnapshot {
            vote,
            meta: snapshot.meta,
            data,
          },
        },
      )
      .await
      .map_err(StreamingError::Unreachable)?;

    match resp {
      RaftRpcResponse::FullSnapshot(r) => {
        r.map_err(|e| StreamingError::Unreachable(Unreachable::new(&e)))
      }
      other => Err(StreamingError::Unreachable(Unreachable::new(&NetErr(
        format!("unexpected response: {other:?}"),
      )))),
    }
  }
}

pub fn parse_p2p_addr(s: &str) -> anyhow::Result<(PeerId, Multiaddr)> {
  let addr: Multiaddr = s.parse().context("invalid multiaddr")?;

  let mut peer: Option<PeerId> = None;
  for p in addr.iter() {
    if let Protocol::P2p(pid) = p {
      peer = Some(pid);
      break;
    }
  }

  let peer = peer.ok_or_else(|| anyhow::anyhow!("multiaddr must include /p2p/<peerid>: {s}"))?;
  Ok((peer, addr))
}

fn ensure_p2p_addr(mut addr: Multiaddr, peer: PeerId) -> Multiaddr {
  if matches!(addr.iter().last(), Some(Protocol::P2p(_))) {
    return addr;
  }
  addr.push(Protocol::P2p(peer.into()));
  addr
}

fn is_loopback_addr(addr: &Multiaddr) -> bool {
  for protocol in addr.iter() {
    match protocol {
      Protocol::Ip4(ip) => return ip.is_loopback(),
      Protocol::Ip6(ip) => return ip.is_loopback(),
      Protocol::Dns(host) | Protocol::Dns4(host) | Protocol::Dns6(host) => {
        if host.eq_ignore_ascii_case("localhost") {
          return true;
        }
      }
      _ => {}
    }
  }
  false
}

use std::{collections::HashMap, future::Future, sync::Arc};

use anyhow::Context;
use libp2p::{Multiaddr, PeerId};
use openraft::{
  BasicNode, RaftNetworkFactory,
  error::{RPCError, Unreachable},
  network::{RPCOption, v2::RaftNetworkV2},
};

use crate::{
  network::{
    rpc::{RaftRpcRequest, RaftRpcResponse},
    swarm::{Libp2pClient, NetErr},
  },
  typ::{
    AppendEntriesRequest, AppendEntriesResponse, NodeId, RpcError, Snapshot, SnapshotResponse,
    StreamingError, Vote, VoteRequest, VoteResponse,
  },
};

#[derive(Clone)]
pub struct Libp2pNetworkFactory {
  client: Libp2pClient,
  node_peers: Arc<tokio::sync::RwLock<HashMap<NodeId, (PeerId, Multiaddr)>>>,
}

impl Libp2pNetworkFactory {
  pub fn new(client: Libp2pClient) -> Self {
    Self {
      client,
      node_peers: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
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

  pub async fn known_nodes(&self) -> Vec<(NodeId, PeerId, Multiaddr)> {
    let map = self.node_peers.read().await;
    map
      .iter()
      .map(|(id, (peer, addr))| (*id, *peer, addr.clone()))
      .collect()
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
}

impl RaftNetworkFactory<openraft_rocksstore::TypeConfig> for Libp2pNetworkFactory {
  type Network = Libp2pConnection;

  async fn new_client(&mut self, target: NodeId, node: &BasicNode) -> Self::Network {
    let _ = self.register_node(target, &node.addr).await;

    Libp2pConnection {
      target,
      factory: self.clone(),
    }
  }
}

impl RaftNetworkV2<openraft_rocksstore::TypeConfig> for Libp2pConnection {
  async fn append_entries(
    &mut self,
    req: AppendEntriesRequest,
    _option: RPCOption,
  ) -> Result<AppendEntriesResponse, RpcError> {
    let peer = self.factory.peer_for(self.target).await?;
    let resp = self
      .factory
      .client
      .request(peer, RaftRpcRequest::AppendEntries(req))
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

  async fn vote(&mut self, req: VoteRequest, _option: RPCOption) -> Result<VoteResponse, RpcError> {
    let peer = self.factory.peer_for(self.target).await?;
    let resp = self
      .factory
      .client
      .request(peer, RaftRpcRequest::Vote(req))
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
        RaftRpcRequest::FullSnapshot {
          vote,
          meta: snapshot.meta,
          data,
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
    if let libp2p::multiaddr::Protocol::P2p(pid) = p {
      peer = Some(pid);
      break;
    }
  }

  let peer = peer.ok_or_else(|| anyhow::anyhow!("multiaddr must include /p2p/<peerid>: {s}"))?;
  Ok((peer, addr))
}

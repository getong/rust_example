use std::{collections::HashMap, sync::Arc};

use anyhow::Context;
use async_trait::async_trait;
use libp2p::{Multiaddr, PeerId, multiaddr::Protocol};
use openraft::BasicNode;

use crate::{
  GroupId, NodeId, Unreachable,
  network::{
    raft_bridge::{P2PNetworkFactory, P2PRaftNetwork, P2PRaftNetworkWrapper},
    rpc::{RaftRpcRequest, RaftRpcResponse},
    swarm::{Libp2pClient, NetErr},
  },
};

#[derive(Clone)]
pub struct Libp2pNetworkFactory {
  client: Libp2pClient,
  node_peers: Arc<tokio::sync::RwLock<HashMap<NodeId, (PeerId, Multiaddr)>>>,
  group_id: Option<GroupId>,
  local_peer_id: PeerId,
}

impl Libp2pNetworkFactory {
  pub fn new(client: Libp2pClient, local_peer_id: PeerId) -> Self {
    Self {
      client,
      node_peers: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
      group_id: None,
      local_peer_id,
    }
  }

  pub fn with_group(&self, group_id: GroupId) -> Self {
    Self {
      client: self.client.clone(),
      node_peers: self.node_peers.clone(),
      group_id: Some(group_id),
      local_peer_id: self.local_peer_id,
    }
  }

  pub async fn register_node(&self, node_id: NodeId, addr: &str) -> anyhow::Result<()> {
    let (peer, maddr) = parse_p2p_addr(addr)?;
    let should_dial = {
      let mut map = self.node_peers.write().await;
      match map.get(&node_id) {
        Some((stored_peer, stored_addr)) if *stored_peer == peer && *stored_addr == maddr => false,
        _ => {
          map.insert(node_id, (peer, maddr.clone()));
          true
        }
      }
    };
    if peer == self.local_peer_id {
      tracing::warn!(
        node_id,
        peer = %peer,
        addr = %maddr,
        "skip self dial in register_node"
      );
      return Ok(());
    }
    if should_dial {
      self.client.dial(maddr).await;
    }
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
    if peer == self.local_peer_id {
      return Err(Unreachable::new(&NetErr(format!(
        "self dial blocked: node_id={node_id}, peer={peer}"
      ))));
    }
    self.client.connect(peer, addr).await?;
    self.client.request(peer, req).await
  }

  async fn peer_addr_for(&self, node_id: NodeId) -> Result<(PeerId, Multiaddr), Unreachable> {
    let map = self.node_peers.read().await;
    map
      .get(&node_id)
      .map(|(peer, addr)| (*peer, addr.clone()))
      .ok_or_else(|| Unreachable::new(&NetErr(format!("unknown target node_id={node_id}"))))
  }
}

struct Libp2pRaftNetwork {
  target: NodeId,
  factory: Libp2pNetworkFactory,
  group_id: GroupId,
}

#[async_trait]
impl P2PNetworkFactory for Libp2pNetworkFactory {
  async fn new_p2p_client(&self, target: NodeId, target_info: BasicNode) -> P2PRaftNetworkWrapper {
    let _ = self.register_node(target, &target_info.addr).await;
    let group_id = self
      .group_id
      .clone()
      .expect("group_id required for raft network");

    P2PRaftNetworkWrapper::new(Libp2pRaftNetwork {
      target,
      factory: self.clone(),
      group_id,
    })
  }
}

#[async_trait]
impl P2PRaftNetwork for Libp2pRaftNetwork {
  fn target(&self) -> NodeId {
    self.target
  }

  fn group_id(&self) -> &GroupId {
    &self.group_id
  }

  async fn send_request(
    &self,
    target: NodeId,
    request: RaftRpcRequest,
  ) -> Result<RaftRpcResponse, Unreachable> {
    self.factory.request(target, request).await
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

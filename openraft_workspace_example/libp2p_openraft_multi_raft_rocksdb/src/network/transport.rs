use std::{collections::HashMap, net::IpAddr, sync::Arc};

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
    let should_dial = self
      .register_node_addr(node_id.clone(), peer, maddr.clone())
      .await;
    if peer == self.local_peer_id {
      tracing::warn!(
        node_id = %node_id,
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

  pub async fn register_discovered_peer(&self, peer: PeerId, addr: Multiaddr) -> bool {
    if peer == self.local_peer_id {
      return false;
    }

    let node_id = NodeId::from(peer.to_string());
    let addr = ensure_p2p_addr(addr, peer);
    if is_undialable_discovered_addr(&addr) {
      tracing::debug!(
        node_id = %node_id,
        peer = %peer,
        addr = %addr,
        "ignore undialable discovered peer address"
      );
      return false;
    }
    self.register_node_addr(node_id, peer, addr).await
  }

  async fn register_node_addr(&self, node_id: NodeId, peer: PeerId, addr: Multiaddr) -> bool {
    let new_is_loopback = is_loopback_addr(&addr);
    let mut map = self.node_peers.write().await;
    match map.get(&node_id) {
      Some((stored_peer, stored_addr)) if *stored_peer == peer && *stored_addr == addr => false,
      Some((stored_peer, stored_addr))
        if *stored_peer == peer && !is_loopback_addr(stored_addr) && new_is_loopback =>
      {
        tracing::info!(
          node_id = %node_id,
          peer = %peer,
          addr = %addr,
          stored_addr = %stored_addr,
          "ignore loopback addr because a non-loopback addr is already known"
        );
        false
      }
      _ => {
        map.insert(node_id.clone(), (peer, addr.clone()));
        tracing::info!(
          node_id = %node_id,
          peer = %peer,
          addr = %addr,
          "registered libp2p node"
        );
        true
      }
    }
  }

  pub async fn update_peer_addr_from_mdns(&self, peer: PeerId, addr: Multiaddr) -> bool {
    let mut map = self.node_peers.write().await;
    for (node_id, (stored_peer, stored_addr)) in map.iter_mut() {
      if *stored_peer != peer {
        continue;
      }

      let candidate = ensure_p2p_addr(addr.clone(), peer);
      if candidate == *stored_addr {
        return true;
      }

      if !should_use_discovered_addr(stored_addr, &candidate) {
        tracing::debug!(
          node_id = %node_id,
          peer = %peer,
          addr = %candidate,
          stored_addr = %stored_addr,
          "ignoring discovered peer address"
        );
        return false;
      }

      if !is_unspecified_addr(stored_addr) {
        return true;
      }

      tracing::info!(
        node_id = %node_id,
        peer = %peer,
        addr = %candidate,
        "updating unspecified peer address from mdns"
      );
      *stored_addr = candidate;
      return true;
    }

    !is_undialable_discovered_addr(&ensure_p2p_addr(addr, peer))
  }

  pub async fn known_nodes(&self) -> Vec<(NodeId, PeerId, Multiaddr)> {
    let map = self.node_peers.read().await;
    map
      .iter()
      .map(|(id, (peer, addr))| (id.clone(), *peer, addr.clone()))
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
    let (peer, addr) = self.peer_addr_for(&node_id).await?;
    if peer == self.local_peer_id {
      return Err(Unreachable::new(&NetErr(format!(
        "self dial blocked: node_id={node_id}, peer={peer}"
      ))));
    }
    if let Err(err) = self.client.connect(peer, addr.clone()).await {
      if is_loopback_addr(&addr) {
        return Err(err);
      }
      tracing::warn!(
        node_id = %node_id,
        peer = %peer,
        addr = %addr,
        error = %err,
        "connect with configured address failed, trying any known address"
      );
      self.client.connect_any(peer).await?;
    }
    self.client.request(peer, req).await
  }

  async fn peer_addr_for(&self, node_id: &NodeId) -> Result<(PeerId, Multiaddr), Unreachable> {
    let map = self.node_peers.read().await;
    map
      .get(node_id)
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
    let _ = self.register_node(target.clone(), &target_info.addr).await;
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
    self.target.clone()
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
  addr_ip(addr).is_some_and(|ip| ip.is_loopback()) || has_localhost_dns(addr)
}

fn is_unspecified_addr(addr: &Multiaddr) -> bool {
  addr_ip(addr).is_some_and(|ip| ip.is_unspecified())
}

fn is_link_local_addr(addr: &Multiaddr) -> bool {
  match addr_ip(addr) {
    Some(IpAddr::V4(ip)) => ip.octets()[0] == 169 && ip.octets()[1] == 254,
    Some(IpAddr::V6(ip)) => (ip.segments()[0] & 0xffc0) == 0xfe80,
    None => false,
  }
}

fn is_undialable_discovered_addr(addr: &Multiaddr) -> bool {
  is_unspecified_addr(addr) || is_link_local_addr(addr)
}

fn should_use_discovered_addr(stored_addr: &Multiaddr, candidate: &Multiaddr) -> bool {
  if is_unspecified_addr(candidate) {
    return false;
  }

  if is_loopback_addr(stored_addr) {
    return is_loopback_addr(candidate);
  }

  if !is_link_local_addr(stored_addr) && is_link_local_addr(candidate) {
    return false;
  }

  true
}

fn addr_ip(addr: &Multiaddr) -> Option<IpAddr> {
  for protocol in addr.iter() {
    match protocol {
      Protocol::Ip4(ip) => return Some(IpAddr::V4(ip)),
      Protocol::Ip6(ip) => return Some(IpAddr::V6(ip)),
      _ => {}
    }
  }
  None
}

fn has_localhost_dns(addr: &Multiaddr) -> bool {
  for protocol in addr.iter() {
    match protocol {
      Protocol::Dns(host) | Protocol::Dns4(host) | Protocol::Dns6(host) => {
        return host.eq_ignore_ascii_case("localhost");
      }
      _ => {}
    }
  }
  false
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use libp2p::identity;
  use tokio::sync::mpsc;

  use super::*;

  fn peer_id() -> PeerId {
    PeerId::from(identity::Keypair::generate_ed25519().public())
  }

  #[tokio::test]
  async fn mdns_does_not_replace_configured_loopback_addr() {
    let (tx, _rx) = mpsc::channel(4);
    let local_peer = peer_id();
    let peer = peer_id();
    let client = Libp2pClient::new(tx, Duration::from_secs(1));
    let network = Libp2pNetworkFactory::new(client, local_peer);
    let node_id = NodeId::from("node-2");
    let configured_addr = format!("/ip4/127.0.0.1/tcp/4002/wss/p2p/{peer}");

    network
      .register_node(node_id.clone(), &configured_addr)
      .await
      .expect("register node");
    let use_discovered = network
      .update_peer_addr_from_mdns(peer, "/ip4/192.168.31.29/tcp/4002/wss".parse().unwrap())
      .await;

    let (_, stored_addr) = network.peer_addr_for(&node_id).await.expect("peer addr");
    assert!(!use_discovered);
    assert_eq!(stored_addr.to_string(), configured_addr);
  }

  #[tokio::test]
  async fn mdns_replaces_unspecified_configured_addr() {
    let (tx, _rx) = mpsc::channel(4);
    let local_peer = peer_id();
    let peer = peer_id();
    let client = Libp2pClient::new(tx, Duration::from_secs(1));
    let network = Libp2pNetworkFactory::new(client, local_peer);
    let node_id = NodeId::from("node-2");
    let configured_addr = format!("/ip4/0.0.0.0/tcp/4002/wss/p2p/{peer}");

    network
      .register_node(node_id.clone(), &configured_addr)
      .await
      .expect("register node");
    let use_discovered = network
      .update_peer_addr_from_mdns(peer, "/ip4/192.168.31.29/tcp/4002/wss".parse().unwrap())
      .await;

    let (_, stored_addr) = network.peer_addr_for(&node_id).await.expect("peer addr");
    assert!(use_discovered);
    assert_eq!(
      stored_addr.to_string(),
      format!("/ip4/192.168.31.29/tcp/4002/wss/p2p/{peer}")
    );
  }

  #[test]
  fn discovered_link_local_addr_is_not_used_for_normal_addr() {
    let peer = peer_id();
    let stored = format!("/ip4/192.168.31.29/tcp/4002/wss/p2p/{peer}")
      .parse()
      .unwrap();
    let candidate = format!("/ip4/169.254.173.129/tcp/4002/wss/p2p/{peer}")
      .parse()
      .unwrap();

    assert!(!should_use_discovered_addr(&stored, &candidate));
  }
}

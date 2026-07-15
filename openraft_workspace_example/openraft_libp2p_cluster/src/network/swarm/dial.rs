//! Dialing and kademlia address-book helpers shared by the full-node and
//! client swarm loops.

use std::{collections::HashMap, time::Duration};

use libp2p::{
  Multiaddr, PeerId, Swarm, kad,
  swarm::dial_opts::{DialOpts, PeerCondition},
};
use tokio::sync::oneshot;

use super::{Behaviour, NetErr, OPENRAFT_CLUSTER_PROVIDER_KEY, state::PendingConnectTable};

pub(crate) const DIAL_RETRY_BACKOFF: Duration = Duration::from_secs(2);

pub(crate) fn ensure_peer_connection(
  swarm: &mut Swarm<Behaviour>,
  pending_connect: &mut PendingConnectTable,
  dial_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
  peer: PeerId,
  addr: Option<Multiaddr>,
  resp: oneshot::Sender<Result<(), NetErr>>,
) {
  if peer == *swarm.local_peer_id() {
    let _ = resp.send(Err(NetErr(format!("self dial blocked: peer={peer}"))));
    return;
  }

  if swarm.is_connected(&peer) {
    let _ = resp.send(Ok(()));
    return;
  }

  if let Some(until) = dial_backoff_until.get(&peer).copied() {
    let now = tokio::time::Instant::now();
    if now < until {
      let wait_ms = (until - now).as_millis();
      let _ = resp.send(Err(NetErr(format!(
        "dial backoff active: peer={peer}, retry_in_ms={wait_ms}"
      ))));
      return;
    }
    dial_backoff_until.remove(&peer);
  }

  let should_dial = pending_connect.add_waiter(peer, resp);

  if let Some(addr) = addr.as_ref() {
    add_kad_address_from_p2p(swarm, addr);
  }
  if should_dial {
    if let Some(addr) = addr {
      dial_known_peer(swarm, peer, addr);
    } else {
      dial_known_peer_any_addr(swarm, peer);
    }
  }
}

pub(crate) fn dial_known_peer(swarm: &mut Swarm<Behaviour>, peer: PeerId, addr: Multiaddr) {
  let dial_opts = DialOpts::peer_id(peer)
    .condition(PeerCondition::DisconnectedAndNotDialing)
    .addresses(vec![addr])
    .build();
  let _ = Swarm::dial(swarm, dial_opts);
}

pub(crate) fn dial_known_peer_any_addr(swarm: &mut Swarm<Behaviour>, peer: PeerId) {
  let dial_opts = DialOpts::peer_id(peer)
    .condition(PeerCondition::DisconnectedAndNotDialing)
    .build();
  let _ = Swarm::dial(swarm, dial_opts);
}

pub(crate) fn dial_peer_addr(swarm: &mut Swarm<Behaviour>, addr: Multiaddr) {
  let peer = addr.iter().last().and_then(|protocol| match protocol {
    libp2p::multiaddr::Protocol::P2p(peer) => Some(peer),
    _ => None,
  });
  if let Some(peer) = peer {
    if peer == *swarm.local_peer_id() {
      tracing::debug!(peer = %peer, addr = %addr, "skip self dial");
      return;
    }
    dial_known_peer(swarm, peer, addr);
  } else {
    let _ = Swarm::dial(swarm, addr);
  }
}

pub(crate) fn add_kad_address_from_p2p(swarm: &mut Swarm<Behaviour>, addr: &Multiaddr) {
  let mut addr = addr.clone();
  let Some(libp2p::multiaddr::Protocol::P2p(peer)) = addr.pop() else {
    return;
  };
  add_kad_peer_address(swarm, peer, addr);
}

pub(crate) fn add_kad_peer_address(swarm: &mut Swarm<Behaviour>, peer: PeerId, addr: Multiaddr) {
  let addr = strip_p2p(addr);
  swarm.behaviour_mut().kad.add_address(&peer, addr);
}

pub(crate) fn strip_p2p(mut addr: Multiaddr) -> Multiaddr {
  if matches!(
    addr.iter().last(),
    Some(libp2p::multiaddr::Protocol::P2p(_))
  ) {
    let _ = addr.pop();
  }
  addr
}

pub(crate) fn leave_openraft_kad(swarm: &mut Swarm<Behaviour>) {
  leave_kad(swarm, &[OPENRAFT_CLUSTER_PROVIDER_KEY.to_string()]);
}

pub(crate) fn leave_kad(swarm: &mut Swarm<Behaviour>, provider_keys: &[String]) {
  for key in provider_keys {
    let record_key = kad::RecordKey::new(key);
    swarm.behaviour_mut().kad.stop_providing(&record_key);
  }
  swarm.behaviour_mut().kad.set_mode(Some(kad::Mode::Client));
  tracing::info!(provider_keys = ?provider_keys, "left kademlia provider mode");
}

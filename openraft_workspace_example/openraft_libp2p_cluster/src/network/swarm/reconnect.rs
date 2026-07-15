//! Automatic reconnect logic driven by the periodic reconnect tick of the
//! full-node swarm loop.

use std::{
  collections::{HashMap, HashSet},
  time::Duration,
};

use libp2p::{PeerId, Swarm};

use super::{
  Behaviour,
  dial::{add_kad_address_from_p2p, dial_peer_addr},
};
use crate::network::transport::Libp2pNetworkFactory;

pub(crate) const RECONNECT_RETRY_BACKOFF: Duration = Duration::from_secs(30);

/// Proactively redial disconnected peers that must stay connected: configured
/// members, bootstrap nodes, and active raft RPC targets (see
/// `Libp2pNetworkFactory::reconnect_targets`). Deliberately NOT all of
/// `known_nodes`: redialing every announced/discovered node would build an
/// O(N^2) full mesh across the cluster and exhaust file descriptors at scale.
/// Non-pinned peers are dialed on demand by the RPC paths and reaped by the
/// swarm idle-connection timeout.
pub(crate) async fn handle_reconnect_tick(
  swarm: &mut Swarm<Behaviour>,
  network: &Libp2pNetworkFactory,
  connected_peers: &HashSet<PeerId>,
  reconnect_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
) {
  let nodes = network.reconnect_targets().await;
  let now = tokio::time::Instant::now();
  for (_node_id, peer_id, addr) in nodes {
    if peer_id == *swarm.local_peer_id() {
      continue;
    }
    if connected_peers.contains(&peer_id) {
      continue;
    }
    if let Some(until) = reconnect_backoff_until.get(&peer_id).copied() {
      if now < until {
        tracing::debug!(
          peer = %peer_id,
          addr = %addr,
          retry_in_ms = (until - now).as_millis(),
          "automatic reconnect backoff active"
        );
        continue;
      }
      reconnect_backoff_until.remove(&peer_id);
    }
    tracing::debug!(
      peer = %peer_id,
      addr = %addr,
      "reconnecting to peer"
    );
    dial_peer_addr(swarm, addr.clone());
    add_kad_address_from_p2p(swarm, &addr);
    reconnect_backoff_until.insert(peer_id, now + RECONNECT_RETRY_BACKOFF);
  }
}

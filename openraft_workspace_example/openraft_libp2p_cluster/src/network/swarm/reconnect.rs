//! Connection topology maintenance driven by the periodic reconnect tick of
//! the full-node swarm loop. Three jobs, in order:
//!
//!   1. redial pinned peers (configured members, bootstrap within its pin TTL, active raft RPC
//!      targets) that dropped;
//!   2. overlay floor — when the node has fewer connections than `overlay_min_connections`, dial a
//!      few random known-alive peers so gossipsub always has enough neighbours to form a healthy
//!      mesh;
//!   3. connection janitor — when the node has more connections than `max_peer_connections`, close
//!      the least-recently-active surplus, sparing pinned peers, gossipsub mesh links, in-flight
//!      dials and recently active RPC partners.
//!
//! Together these keep every node's degree within
//! [overlay_min_connections, max_peer_connections] — O(1) in cluster size —
//! instead of drifting into an O(N^2) full mesh. The address book still
//! knows every node (announcements), but knowing is decoupled from being
//! connected: RPC paths dial on demand.

use std::{
  collections::{HashMap, HashSet},
  hash::{BuildHasher, Hash, Hasher},
  time::Duration,
};

use libp2p::{PeerId, Swarm};

use super::{
  Behaviour,
  dial::{add_kad_address_from_p2p, dial_peer_addr},
  state::SwarmState,
};
use crate::network::transport::Libp2pNetworkFactory;

pub(crate) const RECONNECT_RETRY_BACKOFF: Duration = Duration::from_secs(30);

/// A peer with RPC traffic (or a fresh connection) within this window is
/// never closed by the janitor: it is either mid-conversation or was just
/// dialed on demand and is about to be used.
///
/// Short on purpose. Establishment counts as activity, so this is also the
/// grace every incidental connection (kad walks, gossip probes) gets before
/// it becomes closable; at 60s the background dial churn alone kept most
/// connections perpetually inside the window and the budget could not
/// converge. Peers that actually talk refresh their window on every RPC
/// message, and raft targets are pinned regardless.
const ACTIVITY_PROTECT_WINDOW: Duration = Duration::from_secs(20);

/// Activity entries older than this are dropped so the map stays bounded by
/// the connection budget instead of growing with cluster size.
const ACTIVITY_RETENTION: Duration = Duration::from_secs(900);

/// Overlay-floor dials issued per tick. Deliberately small: with a 12s tick
/// the floor refills quickly without a thundering herd after startup or a
/// mass disconnect.
const OVERLAY_DIALS_PER_TICK: usize = 4;

pub(crate) async fn handle_reconnect_tick(
  swarm: &mut Swarm<Behaviour>,
  network: &Libp2pNetworkFactory,
  state: &mut SwarmState,
  max_peer_connections: usize,
  overlay_min_connections: usize,
) {
  let pinned = redial_pinned_peers(swarm, network, state).await;
  maintain_overlay_floor(swarm, network, state, overlay_min_connections).await;
  enforce_connection_budget(swarm, state, &pinned, max_peer_connections);
}

/// Proactively redial disconnected peers that must stay connected:
/// configured members, bootstrap nodes (within their pin TTL), and active
/// raft RPC targets (see `Libp2pNetworkFactory::reconnect_targets`).
/// Deliberately NOT all of `known_nodes`: redialing every
/// announced/discovered node would build an O(N^2) full mesh across the
/// cluster and exhaust file descriptors at scale. Returns the pinned peer
/// ids so the janitor can spare them.
async fn redial_pinned_peers(
  swarm: &mut Swarm<Behaviour>,
  network: &Libp2pNetworkFactory,
  state: &mut SwarmState,
) -> HashSet<PeerId> {
  let nodes = network.reconnect_targets().await;
  let now = tokio::time::Instant::now();
  let mut pinned = HashSet::with_capacity(nodes.len());
  for (_node_id, peer_id, addr) in nodes {
    if peer_id == *swarm.local_peer_id() {
      continue;
    }
    pinned.insert(peer_id);
    if state.connected_peers.contains(&peer_id) {
      continue;
    }
    if let Some(until) = state.reconnect_backoff_until.get(&peer_id).copied() {
      if now < until {
        tracing::debug!(
          peer = %peer_id,
          addr = %addr,
          retry_in_ms = (until - now).as_millis(),
          "automatic reconnect backoff active"
        );
        continue;
      }
      state.reconnect_backoff_until.remove(&peer_id);
    }
    tracing::debug!(
      peer = %peer_id,
      addr = %addr,
      "reconnecting to peer"
    );
    dial_peer_addr(swarm, addr.clone());
    add_kad_address_from_p2p(swarm, &addr);
    state
      .reconnect_backoff_until
      .insert(peer_id, now + RECONNECT_RETRY_BACKOFF);
  }
  pinned
}

/// Dial a few random known-alive peers while the node sits below the overlay
/// floor. Gossipsub only grafts mesh links over existing connections, so a
/// node that connected to nothing but its pins would leave announce/sync
/// gossip flowing through the bootstrap hub alone. A random bounded overlay
/// makes the gossip graph an expander: connected with overwhelming
/// probability at any cluster size, at O(1) connections per node.
async fn maintain_overlay_floor(
  swarm: &mut Swarm<Behaviour>,
  network: &Libp2pNetworkFactory,
  state: &mut SwarmState,
  overlay_min_connections: usize,
) {
  if overlay_min_connections == 0 || state.connected_peers.len() >= overlay_min_connections {
    return;
  }
  let want = (overlay_min_connections - state.connected_peers.len()).min(OVERLAY_DIALS_PER_TICK);
  let now = tokio::time::Instant::now();
  let local = *swarm.local_peer_id();

  let mut candidates = Vec::new();
  for (_node_id, peer, addr) in network.known_nodes().await {
    if peer == local || state.connected_peers.contains(&peer) {
      continue;
    }
    // Dial backoff covers peers that recently failed to connect.
    if state
      .dial_backoff_until
      .get(&peer)
      .is_some_and(|until| now < *until)
    {
      continue;
    }
    if !network.is_peer_alive(&peer).await {
      continue;
    }
    candidates.push((peer, addr));
  }
  if candidates.is_empty() {
    return;
  }

  // Random selection without a rand dependency: RandomState is seeded
  // per-instantiation, so each tick ranks candidates differently. Random
  // (rather than fixed) neighbours are what make the overlay an expander.
  let seed = std::collections::hash_map::RandomState::new();
  candidates.sort_by_cached_key(|(peer, _)| {
    let mut hasher = seed.build_hasher();
    peer.hash(&mut hasher);
    hasher.finish()
  });

  for (peer, addr) in candidates.into_iter().take(want) {
    tracing::debug!(peer = %peer, addr = %addr, "overlay floor dial");
    dial_peer_addr(swarm, addr.clone());
    add_kad_address_from_p2p(swarm, &addr);
  }
}

/// Close surplus connections above the budget, least-recently-active first.
fn enforce_connection_budget(
  swarm: &mut Swarm<Behaviour>,
  state: &mut SwarmState,
  pinned: &HashSet<PeerId>,
  max_peer_connections: usize,
) {
  let now = tokio::time::Instant::now();
  state
    .last_peer_activity
    .retain(|_, at| now.duration_since(*at) < ACTIVITY_RETENTION);

  if max_peer_connections == 0 || state.connected_peers.len() <= max_peer_connections {
    return;
  }

  let mut protected: HashSet<PeerId> = pinned.clone();
  // Mesh links carry the gossip topology; closing them would churn grafts.
  protected.extend(swarm.behaviour().gossipsub.all_mesh_peers().copied());
  // A dial someone is awaiting must not be reaped between connect and use.
  protected.extend(state.pending_connect.peers().copied());
  for (peer, at) in &state.last_peer_activity {
    if now.duration_since(*at) < ACTIVITY_PROTECT_WINDOW {
      protected.insert(*peer);
    }
  }

  let to_close = select_peers_to_close(
    &state.connected_peers,
    &protected,
    &state.last_peer_activity,
    max_peer_connections,
  );
  if to_close.is_empty() {
    return;
  }
  tracing::info!(
    connected = state.connected_peers.len(),
    budget = max_peer_connections,
    closing = to_close.len(),
    "connection budget exceeded; closing least-recently-active peers"
  );
  metrics::counter!("swarm_janitor_closed_total").increment(to_close.len() as u64);
  for peer in to_close {
    let _ = swarm.disconnect_peer_id(peer);
  }
}

/// Pure selection: which peers to close to get from `connected` down to
/// `budget`, never touching `protected`, least-recently-active first (peers
/// with no recorded activity are considered oldest).
fn select_peers_to_close(
  connected: &HashSet<PeerId>,
  protected: &HashSet<PeerId>,
  last_activity: &HashMap<PeerId, tokio::time::Instant>,
  budget: usize,
) -> Vec<PeerId> {
  let excess = connected.len().saturating_sub(budget);
  if excess == 0 {
    return Vec::new();
  }
  let mut candidates: Vec<PeerId> = connected
    .iter()
    .filter(|peer| !protected.contains(*peer))
    .copied()
    .collect();
  // `None` sorts before `Some`, so never-active peers close first.
  candidates.sort_by_key(|peer| last_activity.get(peer).copied());
  candidates.truncate(excess);
  candidates
}

#[cfg(test)]
mod tests {
  use super::*;

  fn peers(n: usize) -> Vec<PeerId> {
    (0 .. n).map(|_| PeerId::random()).collect()
  }

  #[tokio::test(start_paused = true)]
  async fn respects_budget_and_protection() {
    // The paused clock starts near zero; move it forward so subtracting an
    // activity age cannot underflow the Instant.
    tokio::time::advance(Duration::from_secs(3600)).await;
    let all = peers(6);
    let connected: HashSet<PeerId> = all.iter().copied().collect();
    let protected: HashSet<PeerId> = all[.. 2].iter().copied().collect();
    let mut activity = HashMap::new();
    let now = tokio::time::Instant::now();
    // all[2] and all[3] have recorded activity; all[4] and all[5] never
    // spoke, so they are considered oldest and close first.
    activity.insert(all[2], now);
    activity.insert(all[3], now - Duration::from_secs(300));

    let to_close = select_peers_to_close(&connected, &protected, &activity, 4);
    assert_eq!(to_close.len(), 2);
    assert!(!to_close.contains(&all[0]));
    assert!(!to_close.contains(&all[1]));
    assert!(to_close.contains(&all[4]));
    assert!(to_close.contains(&all[5]));
  }

  #[test]
  fn no_close_under_budget() {
    let all = peers(3);
    let connected: HashSet<PeerId> = all.iter().copied().collect();
    assert!(select_peers_to_close(&connected, &HashSet::new(), &HashMap::new(), 3).is_empty());
    assert!(select_peers_to_close(&connected, &HashSet::new(), &HashMap::new(), 10).is_empty());
  }

  #[test]
  fn protection_beats_budget() {
    let all = peers(4);
    let connected: HashSet<PeerId> = all.iter().copied().collect();
    let protected: HashSet<PeerId> = all[.. 3].iter().copied().collect();
    // Excess is 4, but only one peer is unprotected: the budget is a soft
    // cap — protection wins.
    let to_close = select_peers_to_close(&connected, &protected, &HashMap::new(), 0);
    assert_eq!(to_close, vec![all[3]]);
  }
}

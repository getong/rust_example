//! Connection lifecycle event handling: established/closed transitions and
//! outgoing dial failures with log throttling.

use std::{
  collections::{HashMap, HashSet},
  fmt,
  time::Duration,
};

use libp2p::{PeerId, Swarm};

use crate::network::swarm::{
  Behaviour, NetErr, dial::DIAL_RETRY_BACKOFF, state::PendingConnectTable,
};

const OUTGOING_FAILURE_LOG_BACKOFF: Duration = Duration::from_secs(30);

pub(crate) fn handle_connection_established(
  swarm: &mut Swarm<Behaviour>,
  pending_connect: &mut PendingConnectTable,
  connected_peers: &mut HashSet<PeerId>,
  dial_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
  peer_id: PeerId,
) {
  connected_peers.insert(peer_id);
  dial_backoff_until.remove(&peer_id);
  pending_connect.finish(peer_id, Ok(()));
  swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
}

pub(crate) fn handle_connection_closed<E: fmt::Display>(
  swarm: &mut Swarm<Behaviour>,
  connected_peers: &mut HashSet<PeerId>,
  peer_id: PeerId,
  num_established: u32,
  cause: Option<E>,
) {
  if num_established == 0 {
    connected_peers.remove(&peer_id);
    swarm
      .behaviour_mut()
      .gossipsub
      .remove_explicit_peer(&peer_id);
    if let Some(cause) = cause {
      tracing::warn!(peer = %peer_id, error = %cause, "connection closed");
    } else {
      tracing::info!(peer = %peer_id, "connection closed");
    }
  }
}

pub(crate) fn handle_outgoing_connection_error<E: fmt::Display>(
  swarm: &mut Swarm<Behaviour>,
  pending_connect: &mut PendingConnectTable,
  dial_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
  failure_log_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
  peer_id: Option<PeerId>,
  error: E,
) {
  let Some(peer_id) = peer_id else {
    tracing::warn!(
      error = %error,
      file = file!(),
      line = line!(),
      "outgoing connection failed"
    );
    return;
  };

  if swarm.is_connected(&peer_id) {
    return;
  }

  let has_waiters = pending_connect.contains(&peer_id);
  dial_backoff_until.insert(peer_id, tokio::time::Instant::now() + DIAL_RETRY_BACKOFF);
  if has_waiters && should_log_outgoing_failure(peer_id, failure_log_backoff_until) {
    tracing::warn!(
      peer = %peer_id,
      error = %error,
      file = file!(),
      line = line!(),
      "outgoing connection failed"
    );
  } else {
    tracing::debug!(
      peer = %peer_id,
      error = %error,
      file = file!(),
      line = line!(),
      has_waiters,
      "outgoing connection failed; suppressing warning"
    );
  }
  pending_connect.finish(peer_id, Err(NetErr(format!("dial failed: {error}"))));
}

fn should_log_outgoing_failure(
  peer_id: PeerId,
  failure_log_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
) -> bool {
  let now = tokio::time::Instant::now();
  if let Some(until) = failure_log_backoff_until.get(&peer_id).copied() {
    if now < until {
      return false;
    }
    failure_log_backoff_until.remove(&peer_id);
  }
  failure_log_backoff_until.insert(peer_id, now + OUTGOING_FAILURE_LOG_BACKOFF);
  true
}

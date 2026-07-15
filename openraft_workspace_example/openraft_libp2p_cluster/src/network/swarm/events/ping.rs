//! Ping event handling: track per-connection failures and force-close
//! connections to unresponsive peers.

use std::collections::HashMap;

use libp2p::{Swarm, ping, swarm::ConnectionId};

use crate::network::swarm::Behaviour;

/// Consecutive ping failures on one connection before it is force-closed.
/// A hung peer (SIGSTOP, long GC pause) keeps its TCP connection alive at
/// the kernel level, so peers still see it as "connected" even though it
/// answers nothing. The membership guard keys voter liveness off libp2p
/// connectedness — without this cutoff a hung voter is never replaced.
const PING_FAILURE_DISCONNECT_THRESHOLD: u32 = 3;

pub(crate) fn handle_ping_event(
  swarm: &mut Swarm<Behaviour>,
  ping_failures: &mut HashMap<ConnectionId, u32>,
  event: ping::Event,
) {
  let ping::Event {
    peer,
    connection,
    result,
  } = event;
  match result {
    Ok(rtt) => {
      ping_failures.remove(&connection);
      tracing::debug!(peer = %peer, rtt = ?rtt, "ping ok");
    }
    Err(err) => {
      let failures = ping_failures.entry(connection).or_insert(0);
      *failures += 1;
      tracing::warn!(peer = %peer, error = ?err, failures = *failures, "ping failed");
      if *failures >= PING_FAILURE_DISCONNECT_THRESHOLD {
        ping_failures.remove(&connection);
        tracing::warn!(
          peer = %peer,
          ?connection,
          "closing unresponsive connection after repeated ping failures"
        );
        swarm.close_connection(connection);
      }
    }
  }
}

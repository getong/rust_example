//! Per-peer outbound RPC guard: bulkhead + circuit breaker.
//!
//! Outbound RPCs share one swarm loop and (on the receiving side) one
//! dispatch semaphore, so a single slow or dead peer can otherwise absorb an
//! unbounded number of in-flight requests — most visibly when the membership
//! guard replaces a dead voter and every raft RPC to it hangs until timeout.
//!
//! Two mechanisms, both per target peer:
//!   - **Bulkhead**: at most [`PER_PEER_MAX_CONCURRENT_RPCS`] RPCs in flight to one peer; excess
//!     callers get an immediate typed failure instead of queueing.
//!   - **Circuit breaker**: after [`CIRCUIT_BREAKER_FAILURE_THRESHOLD`] consecutive failures the
//!     circuit opens for [`CIRCUIT_BREAKER_OPEN_DURATION`]; requests are rejected without touching
//!     the network. After the cooldown the circuit is half-open: traffic flows again, but a single
//!     failure re-opens it immediately, while one success closes it fully.
//!
//! Rejections surface as `Unreachable` to callers, which every RPC consumer
//! already handles with its own retry/backoff (openraft included).

use std::{
  collections::HashMap,
  fmt,
  sync::{
    Arc, Mutex,
    atomic::{AtomicU32, AtomicU64, Ordering},
  },
  time::Duration,
};

use libp2p::PeerId;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, TryAcquireError};

/// Max concurrently in-flight outbound RPCs per target peer. Well below the
/// receiver-side inbound dispatch cap (256), so even a handful of slow peers
/// cannot monopolize the shared resources.
const PER_PEER_MAX_CONCURRENT_RPCS: usize = 32;
/// Consecutive failures to one peer before its circuit opens.
const CIRCUIT_BREAKER_FAILURE_THRESHOLD: u32 = 5;
/// How long an open circuit rejects requests before going half-open.
const CIRCUIT_BREAKER_OPEN_DURATION: Duration = Duration::from_secs(10);

/// Why an RPC to a peer was rejected without being sent.
#[derive(Debug, Clone)]
pub enum PeerRpcRejection {
  /// The peer's circuit is open after repeated consecutive failures.
  CircuitOpen { retry_in: Duration },
  /// The per-peer concurrency limit is fully in use.
  BulkheadFull,
}

impl fmt::Display for PeerRpcRejection {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::CircuitOpen { retry_in } => write!(
        f,
        "circuit open after repeated failures; retry_in_ms={}",
        retry_in.as_millis()
      ),
      Self::BulkheadFull => write!(
        f,
        "per-peer rpc concurrency limit ({PER_PEER_MAX_CONCURRENT_RPCS}) exhausted"
      ),
    }
  }
}

impl std::error::Error for PeerRpcRejection {}

struct PeerGuardState {
  bulkhead: Arc<Semaphore>,
  consecutive_failures: AtomicU32,
  /// Circuit state, encoded as "millis since guard creation until which the
  /// circuit stays open". 0 = closed. A non-zero value in the past means
  /// half-open: traffic flows, but one failure re-opens immediately.
  open_until_ms: AtomicU64,
}

impl PeerGuardState {
  fn new() -> Self {
    Self {
      bulkhead: Arc::new(Semaphore::new(PER_PEER_MAX_CONCURRENT_RPCS)),
      consecutive_failures: AtomicU32::new(0),
      open_until_ms: AtomicU64::new(0),
    }
  }
}

/// Guards all outbound RPC traffic of one node, keyed by target peer.
pub struct PeerRpcGuard {
  created: tokio::time::Instant,
  peers: Mutex<HashMap<PeerId, Arc<PeerGuardState>>>,
}

impl Default for PeerRpcGuard {
  fn default() -> Self {
    Self {
      created: tokio::time::Instant::now(),
      peers: Mutex::new(HashMap::new()),
    }
  }
}

impl PeerRpcGuard {
  fn now_ms(&self) -> u64 {
    self.created.elapsed().as_millis() as u64
  }

  fn state_for(&self, peer: PeerId) -> Arc<PeerGuardState> {
    let mut peers = self.peers.lock().expect("peer guard lock poisoned");
    peers
      .entry(peer)
      .or_insert_with(|| Arc::new(PeerGuardState::new()))
      .clone()
  }

  /// Admit an RPC to `peer`, or reject it without touching the network. The
  /// returned permit must be fed the outcome (`record_success` /
  /// `record_failure`) so the circuit tracks consecutive failures.
  pub fn try_acquire(&self, peer: PeerId) -> Result<PeerRpcPermit, PeerRpcRejection> {
    let state = self.state_for(peer);

    let open_until_ms = state.open_until_ms.load(Ordering::Acquire);
    if open_until_ms != 0 {
      let now_ms = self.now_ms();
      if now_ms < open_until_ms {
        metrics::counter!("rpc_circuit_open_reject_total").increment(1);
        return Err(PeerRpcRejection::CircuitOpen {
          retry_in: Duration::from_millis(open_until_ms - now_ms),
        });
      }
      // Cooldown elapsed: half-open. Leave open_until_ms set so a failure
      // re-opens immediately; a success clears it.
    }

    let permit = match state.bulkhead.clone().try_acquire_owned() {
      Ok(permit) => permit,
      Err(TryAcquireError::NoPermits) | Err(TryAcquireError::Closed) => {
        metrics::counter!("rpc_bulkhead_reject_total").increment(1);
        return Err(PeerRpcRejection::BulkheadFull);
      }
    };

    Ok(PeerRpcPermit {
      _permit: permit,
      state,
      open_at_ms: self.now_ms() + CIRCUIT_BREAKER_OPEN_DURATION.as_millis() as u64,
    })
  }
}

/// In-flight RPC slot for one peer. Dropping it releases the bulkhead slot;
/// the caller reports the outcome so the circuit breaker can act on it.
pub struct PeerRpcPermit {
  _permit: OwnedSemaphorePermit,
  state: Arc<PeerGuardState>,
  /// Precomputed "open until" timestamp to store if this RPC fails and trips
  /// the circuit.
  open_at_ms: u64,
}

impl PeerRpcPermit {
  pub fn record_success(&self) {
    self.state.consecutive_failures.store(0, Ordering::Release);
    self.state.open_until_ms.store(0, Ordering::Release);
  }

  pub fn record_failure(&self) {
    let half_open = self.state.open_until_ms.load(Ordering::Acquire) != 0;
    let failures = self
      .state
      .consecutive_failures
      .fetch_add(1, Ordering::AcqRel)
      + 1;
    if half_open || failures >= CIRCUIT_BREAKER_FAILURE_THRESHOLD {
      self
        .state
        .open_until_ms
        .store(self.open_at_ms, Ordering::Release);
      self.state.consecutive_failures.store(0, Ordering::Release);
      metrics::counter!("rpc_circuit_opened_total").increment(1);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn peer() -> PeerId {
    PeerId::from(libp2p::identity::Keypair::generate_ed25519().public())
  }

  #[tokio::test(start_paused = true)]
  async fn circuit_opens_after_consecutive_failures_and_recovers() {
    let guard = PeerRpcGuard::default();
    let target = peer();

    for _ in 0 .. CIRCUIT_BREAKER_FAILURE_THRESHOLD {
      let permit = guard.try_acquire(target).expect("closed circuit admits");
      permit.record_failure();
    }

    // Open: rejected without a permit.
    assert!(matches!(
      guard.try_acquire(target),
      Err(PeerRpcRejection::CircuitOpen { .. })
    ));

    // After the cooldown the circuit is half-open: one probe is admitted.
    tokio::time::advance(CIRCUIT_BREAKER_OPEN_DURATION + Duration::from_millis(1)).await;
    let probe = guard.try_acquire(target).expect("half-open admits a probe");

    // A half-open failure re-opens immediately (no need for a full streak).
    probe.record_failure();
    assert!(matches!(
      guard.try_acquire(target),
      Err(PeerRpcRejection::CircuitOpen { .. })
    ));

    // Cooldown again, then a success closes the circuit fully.
    tokio::time::advance(CIRCUIT_BREAKER_OPEN_DURATION + Duration::from_millis(1)).await;
    let probe = guard.try_acquire(target).expect("half-open admits a probe");
    probe.record_success();
    let permit = guard.try_acquire(target).expect("closed circuit admits");
    permit.record_failure();
    // One failure after recovery must not re-open the circuit.
    assert!(guard.try_acquire(target).is_ok());
  }

  #[tokio::test]
  async fn bulkhead_rejects_when_full_and_releases_on_drop() {
    let guard = PeerRpcGuard::default();
    let target = peer();

    let permits: Vec<_> = (0 .. PER_PEER_MAX_CONCURRENT_RPCS)
      .map(|_| guard.try_acquire(target).expect("slot available"))
      .collect();
    assert!(matches!(
      guard.try_acquire(target),
      Err(PeerRpcRejection::BulkheadFull)
    ));

    drop(permits);
    assert!(guard.try_acquire(target).is_ok());
  }

  #[tokio::test]
  async fn peers_are_isolated() {
    let guard = PeerRpcGuard::default();
    let slow = peer();
    let healthy = peer();

    for _ in 0 .. CIRCUIT_BREAKER_FAILURE_THRESHOLD {
      guard
        .try_acquire(slow)
        .expect("closed circuit admits")
        .record_failure();
    }
    assert!(matches!(
      guard.try_acquire(slow),
      Err(PeerRpcRejection::CircuitOpen { .. })
    ));
    // The slow peer's open circuit must not affect other peers.
    assert!(guard.try_acquire(healthy).is_ok());
  }
}

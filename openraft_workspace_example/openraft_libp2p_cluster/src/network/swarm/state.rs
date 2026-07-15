//! Mutable bookkeeping owned by a swarm loop.
//!
//! Each pending table is its own type so handlers speak in intent
//! (`complete`, `finish`, `fail_all`) instead of raw map plumbing, and the
//! shutdown path can fail everything in one call.

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use libp2p::{PeerId, kad, request_response::OutboundRequestId, swarm::ConnectionId};
use tokio::sync::{Semaphore, oneshot};

use super::NetErr;
use crate::network::{openraft_sync::OpenRaftSyncState, rpc::UnifiedRpcResponse};

/// Cap on concurrently executing inbound RPC dispatches (all protocols).
/// Every inbound request spawns a task; without a cap, an election storm or
/// a large cluster rejoin can proliferate dispatch tasks without bound. The
/// permit is acquired inside the spawned task so the swarm loop itself never
/// blocks; excess requests queue on the semaphore instead of all executing
/// at once.
pub(crate) const MAX_CONCURRENT_INBOUND_DISPATCHES: usize = 256;

/// Pending outbound RPC requests keyed by their request id.
#[derive(Default)]
pub(crate) struct PendingRpcTable {
  inner: HashMap<OutboundRequestId, oneshot::Sender<Result<UnifiedRpcResponse, NetErr>>>,
}

impl PendingRpcTable {
  pub(crate) fn insert(
    &mut self,
    id: OutboundRequestId,
    tx: oneshot::Sender<Result<UnifiedRpcResponse, NetErr>>,
  ) {
    self.inner.insert(id, tx);
  }

  /// Deliver `result` to the waiter of `id`, if it is still pending.
  pub(crate) fn complete(
    &mut self,
    id: &OutboundRequestId,
    result: Result<UnifiedRpcResponse, NetErr>,
  ) {
    if let Some(tx) = self.inner.remove(id) {
      let _ = tx.send(result);
    }
  }

  pub(crate) fn len(&self) -> usize {
    self.inner.len()
  }

  pub(crate) fn fail_all(&mut self, reason: &str) {
    for (_, tx) in self.inner.drain() {
      let _ = tx.send(Err(NetErr(reason.to_string())));
    }
  }
}

/// Waiters for `EnsureConnection{,Any}` commands, grouped per peer so one
/// in-flight dial serves every concurrent caller.
#[derive(Default)]
pub(crate) struct PendingConnectTable {
  inner: HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>>,
}

impl PendingConnectTable {
  /// Register a waiter for `peer`. Returns `true` when this is the first
  /// waiter — i.e. the caller should start the dial.
  pub(crate) fn add_waiter(
    &mut self,
    peer: PeerId,
    tx: oneshot::Sender<Result<(), NetErr>>,
  ) -> bool {
    match self.inner.get_mut(&peer) {
      Some(waiters) => {
        waiters.push(tx);
        false
      }
      None => {
        self.inner.insert(peer, vec![tx]);
        true
      }
    }
  }

  pub(crate) fn contains(&self, peer: &PeerId) -> bool {
    self.inner.contains_key(peer)
  }

  /// Resolve every waiter for `peer` with `result`.
  pub(crate) fn finish(&mut self, peer: PeerId, result: Result<(), NetErr>) {
    let Some(waiters) = self.inner.remove(&peer) else {
      return;
    };
    for waiter in waiters {
      let _ = waiter.send(result.clone());
    }
  }

  pub(crate) fn len(&self) -> usize {
    self.inner.len()
  }

  pub(crate) fn fail_all(&mut self, reason: &str) {
    for (_, waiters) in self.inner.drain() {
      for waiter in waiters {
        let _ = waiter.send(Err(NetErr(reason.to_string())));
      }
    }
  }
}

pub(crate) struct GetProvidersState {
  pub(crate) providers: HashSet<PeerId>,
  pub(crate) resp: oneshot::Sender<Result<HashSet<PeerId>, NetErr>>,
}

/// In-flight kademlia queries (start_providing / get_providers) keyed by
/// query id.
#[derive(Default)]
pub(crate) struct PendingKadTable {
  pub(crate) start_providing: HashMap<kad::QueryId, oneshot::Sender<Result<(), NetErr>>>,
  pub(crate) get_providers: HashMap<kad::QueryId, GetProvidersState>,
}

impl PendingKadTable {
  pub(crate) fn fail_all(&mut self, reason: &str) {
    for (_, resp) in self.start_providing.drain() {
      let _ = resp.send(Err(NetErr(reason.to_string())));
    }
    for (_, state) in self.get_providers.drain() {
      let _ = state.resp.send(Err(NetErr(reason.to_string())));
    }
  }
}

/// All mutable bookkeeping owned by a swarm loop. Consolidated into one
/// struct so command/event handlers take a single state parameter instead of
/// ~19 loose `&mut` arguments, and adding a new pending table touches one
/// place instead of every handler signature.
pub(crate) struct SwarmState {
  /// Bounds concurrently executing inbound dispatch tasks; see
  /// `MAX_CONCURRENT_INBOUND_DISPATCHES`.
  pub(crate) inbound_dispatch_limit: Arc<Semaphore>,
  pub(crate) pending_rpc: PendingRpcTable,
  pub(crate) pending_connect: PendingConnectTable,
  pub(crate) pending_kad: PendingKadTable,
  pub(crate) openraft_sync: OpenRaftSyncState,
  pub(crate) connected_peers: HashSet<PeerId>,
  pub(crate) dial_backoff_until: HashMap<PeerId, tokio::time::Instant>,
  pub(crate) reconnect_backoff_until: HashMap<PeerId, tokio::time::Instant>,
  pub(crate) outgoing_failure_log_backoff_until: HashMap<PeerId, tokio::time::Instant>,
  pub(crate) ping_failures: HashMap<ConnectionId, u32>,
}

impl Default for SwarmState {
  fn default() -> Self {
    Self {
      inbound_dispatch_limit: Arc::new(Semaphore::new(MAX_CONCURRENT_INBOUND_DISPATCHES)),
      pending_rpc: PendingRpcTable::default(),
      pending_connect: PendingConnectTable::default(),
      pending_kad: PendingKadTable::default(),
      openraft_sync: OpenRaftSyncState::default(),
      connected_peers: HashSet::new(),
      dial_backoff_until: HashMap::new(),
      reconnect_backoff_until: HashMap::new(),
      outgoing_failure_log_backoff_until: HashMap::new(),
      ping_failures: HashMap::new(),
    }
  }
}

impl SwarmState {
  /// Fail every in-flight request with `reason`. Called on shutdown and when
  /// the swarm stream or command channel ends.
  pub(crate) fn fail_all_pending(&mut self, reason: &'static str) {
    self.pending_rpc.fail_all(reason);
    self.pending_connect.fail_all(reason);
    self.pending_kad.fail_all(reason);
  }
}

/// Export the pending-request depth plus connection and dispatch-permit
/// gauges. Called on the (12s) reconnect tick, so gauge freshness costs
/// nothing on the hot event path.
pub(crate) fn record_swarm_gauges(state: &SwarmState) {
  metrics::gauge!("swarm_pending_requests").set(state.pending_rpc.len() as f64);
  metrics::gauge!("swarm_pending_connects").set(state.pending_connect.len() as f64);
  metrics::gauge!("swarm_connected_peers").set(state.connected_peers.len() as f64);
  metrics::gauge!("swarm_inbound_dispatch_available_permits")
    .set(state.inbound_dispatch_limit.available_permits() as f64);
}

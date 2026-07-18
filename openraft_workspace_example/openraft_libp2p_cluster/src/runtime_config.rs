//! Hot-reloadable runtime configuration.
//!
//! Non-critical tunables (announce cadence, membership-guard timeout) live
//! in a process-wide `ArcSwap` so they can be changed at runtime via
//! `GET/POST /config` without a restart. Critical settings (raft timeouts,
//! listen addresses, group layout) intentionally stay CLI-only: changing
//! them safely requires a coordinated restart.
//!
//! Consumers load the current value at each use site (guard tick, announce
//! round), so an update takes effect on the next cycle — no watcher plumbing
//! needed. Reads are lock-free.

use std::{sync::Arc, time::Duration};

use arc_swap::ArcSwap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

/// Lower bounds accepted by [`apply_patch`]. Below these the features stop
/// working sanely (announce storms; guard replacing nodes on a blip).
const MIN_NODE_ANNOUNCE_INTERVAL_SECS: u64 = 1;
const MIN_VOTER_REPLACE_TIMEOUT_SECS: u64 = 5;
const MIN_WORKER_LEASE_INTERVAL_SECS: u64 = 1;
const MIN_TASK_STUCK_REQUEUE_SECS: u64 = 5;
const MIN_WASM_SYNC_ANNOUNCE_INTERVAL_SECS: u64 = 1;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RuntimeConfig {
  /// Base node-announce interval in seconds, before cluster-size scaling
  /// (see `adaptive_announce_interval`).
  pub node_announce_interval_secs: u64,
  /// Seconds a member must stay unreachable before the membership guard
  /// acts on it (voter replaced / learner removed). Also the prune timeout
  /// for dead non-member nodes in the address book.
  pub voter_replace_timeout_secs: u64,
  /// Whether RocksDB writes (raft log + state machine) fsync the WAL on
  /// every write batch.
  ///
  /// - `true` (default): every committed batch is durable across power loss and OS crashes — the
  ///   strongest guarantee, at the cost of one fsync per raft write batch.
  /// - `false`: writes only reach the OS page cache; a machine crash can lose the tail of
  ///   acknowledged writes ON THIS NODE. Raft replication still protects committed entries as long
  ///   as a quorum keeps them, so this trades single-node durability for write throughput. Do not
  ///   disable on more than a minority of a group's voters.
  ///
  /// Hot-reloadable via `POST /config`; takes effect on the next write.
  pub rocksdb_sync_writes: bool,
  /// Worker lease TTL in seconds: how long a lease stays valid after a
  /// renewal. Raise it for deployments whose control plane forwards renewals
  /// slowly; the scheduler treats an expired lease as a dead worker and
  /// requeues its tasks.
  pub worker_lease_ttl_secs: u64,
  /// How often each worker renews its lease. Must stay well below the TTL
  /// (validated: interval < ttl) so a single missed renewal round does not
  /// expire the lease.
  pub worker_lease_interval_secs: u64,
  /// A task stuck in Assigned/Running on a LIVE worker for longer than this
  /// is returned to the queue. Raise it for long-running task kinds (high
  /// fuel budget WASM, slow webhooks) so they are not requeued mid-run.
  pub task_stuck_requeue_secs: u64,
  /// Wasm module inventory announce cadence (gossipsub) in seconds. Also
  /// the effective retry cadence for stalled module downloads. Lower it on
  /// small/fast clusters for snappier propagation; raise it on large
  /// clusters or slow networks to cut gossip traffic.
  pub wasm_sync_announce_interval_secs: u64,
}

impl Default for RuntimeConfig {
  fn default() -> Self {
    Self {
      node_announce_interval_secs: crate::app::NODE_ANNOUNCE_INTERVAL.as_secs(),
      voter_replace_timeout_secs: 300,
      rocksdb_sync_writes: true,
      worker_lease_ttl_secs: 30,
      worker_lease_interval_secs: 10,
      task_stuck_requeue_secs: 60,
      wasm_sync_announce_interval_secs: crate::wasm_sync::service::WASM_SYNC_ANNOUNCE_INTERVAL
        .as_secs(),
    }
  }
}

impl RuntimeConfig {
  pub fn node_announce_interval(&self) -> Duration {
    Duration::from_secs(self.node_announce_interval_secs)
  }

  pub fn voter_replace_timeout(&self) -> Duration {
    Duration::from_secs(self.voter_replace_timeout_secs)
  }

  pub fn worker_lease_interval(&self) -> Duration {
    Duration::from_secs(self.worker_lease_interval_secs)
  }

  pub fn wasm_sync_announce_interval(&self) -> Duration {
    Duration::from_secs(self.wasm_sync_announce_interval_secs)
  }
}

/// Partial update: only the provided fields change.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RuntimeConfigPatch {
  pub node_announce_interval_secs: Option<u64>,
  pub voter_replace_timeout_secs: Option<u64>,
  pub rocksdb_sync_writes: Option<bool>,
  pub worker_lease_ttl_secs: Option<u64>,
  pub worker_lease_interval_secs: Option<u64>,
  pub task_stuck_requeue_secs: Option<u64>,
  pub wasm_sync_announce_interval_secs: Option<u64>,
}

static RUNTIME_CONFIG: Lazy<ArcSwap<RuntimeConfig>> =
  Lazy::new(|| ArcSwap::from_pointee(RuntimeConfig::default()));

/// Current configuration (lock-free load).
pub fn current() -> Arc<RuntimeConfig> {
  RUNTIME_CONFIG.load_full()
}

/// Install the startup configuration (from CLI options).
pub fn install(config: RuntimeConfig) {
  RUNTIME_CONFIG.store(Arc::new(config));
}

/// Validate and atomically apply a partial update; returns the new config.
pub fn apply_patch(patch: RuntimeConfigPatch) -> Result<Arc<RuntimeConfig>, String> {
  if let Some(secs) = patch.node_announce_interval_secs
    && secs < MIN_NODE_ANNOUNCE_INTERVAL_SECS
  {
    return Err(format!(
      "node_announce_interval_secs must be >= {MIN_NODE_ANNOUNCE_INTERVAL_SECS}, got {secs}"
    ));
  }
  if let Some(secs) = patch.voter_replace_timeout_secs
    && secs < MIN_VOTER_REPLACE_TIMEOUT_SECS
  {
    return Err(format!(
      "voter_replace_timeout_secs must be >= {MIN_VOTER_REPLACE_TIMEOUT_SECS}, got {secs}"
    ));
  }
  if let Some(secs) = patch.worker_lease_interval_secs
    && secs < MIN_WORKER_LEASE_INTERVAL_SECS
  {
    return Err(format!(
      "worker_lease_interval_secs must be >= {MIN_WORKER_LEASE_INTERVAL_SECS}, got {secs}"
    ));
  }
  if let Some(secs) = patch.task_stuck_requeue_secs
    && secs < MIN_TASK_STUCK_REQUEUE_SECS
  {
    return Err(format!(
      "task_stuck_requeue_secs must be >= {MIN_TASK_STUCK_REQUEUE_SECS}, got {secs}"
    ));
  }
  if let Some(secs) = patch.wasm_sync_announce_interval_secs
    && secs < MIN_WASM_SYNC_ANNOUNCE_INTERVAL_SECS
  {
    return Err(format!(
      "wasm_sync_announce_interval_secs must be >= {MIN_WASM_SYNC_ANNOUNCE_INTERVAL_SECS}, got \
       {secs}"
    ));
  }
  // The TTL must comfortably outlive the renewal interval, or a single slow
  // renewal expires the lease and the scheduler requeues live workers' tasks.
  {
    let current = current();
    let ttl = patch
      .worker_lease_ttl_secs
      .unwrap_or(current.worker_lease_ttl_secs);
    let interval = patch
      .worker_lease_interval_secs
      .unwrap_or(current.worker_lease_interval_secs);
    if ttl <= interval {
      return Err(format!(
        "worker_lease_ttl_secs ({ttl}) must be greater than worker_lease_interval_secs          \
         ({interval})"
      ));
    }
  }

  RUNTIME_CONFIG.rcu(|current| {
    let mut next = (**current).clone();
    if let Some(secs) = patch.node_announce_interval_secs {
      next.node_announce_interval_secs = secs;
    }
    if let Some(secs) = patch.voter_replace_timeout_secs {
      next.voter_replace_timeout_secs = secs;
    }
    if let Some(sync) = patch.rocksdb_sync_writes {
      next.rocksdb_sync_writes = sync;
    }
    if let Some(secs) = patch.worker_lease_ttl_secs {
      next.worker_lease_ttl_secs = secs;
    }
    if let Some(secs) = patch.worker_lease_interval_secs {
      next.worker_lease_interval_secs = secs;
    }
    if let Some(secs) = patch.task_stuck_requeue_secs {
      next.task_stuck_requeue_secs = secs;
    }
    if let Some(secs) = patch.wasm_sync_announce_interval_secs {
      next.wasm_sync_announce_interval_secs = secs;
    }
    next
  });

  let new = current();
  tracing::info!(config = ?new, "runtime config updated");
  Ok(new)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn patch_updates_only_provided_fields() {
    install(RuntimeConfig {
      node_announce_interval_secs: 20,
      voter_replace_timeout_secs: 300,
      ..RuntimeConfig::default()
    });

    let updated = apply_patch(RuntimeConfigPatch {
      voter_replace_timeout_secs: Some(60),
      ..Default::default()
    })
    .expect("apply patch");
    assert_eq!(updated.node_announce_interval_secs, 20);
    assert_eq!(updated.voter_replace_timeout_secs, 60);
    assert_eq!(current().voter_replace_timeout_secs, 60);
  }

  #[test]
  fn wasm_sync_announce_interval_is_patchable_and_bounded() {
    let updated = apply_patch(RuntimeConfigPatch {
      wasm_sync_announce_interval_secs: Some(120),
      ..Default::default()
    })
    .expect("apply patch");
    assert_eq!(updated.wasm_sync_announce_interval_secs, 120);
    assert_eq!(
      updated.wasm_sync_announce_interval(),
      Duration::from_secs(120)
    );
    assert!(
      apply_patch(RuntimeConfigPatch {
        wasm_sync_announce_interval_secs: Some(0),
        ..Default::default()
      })
      .is_err()
    );
  }

  #[test]
  fn patch_rejects_out_of_range_values() {
    assert!(
      apply_patch(RuntimeConfigPatch {
        node_announce_interval_secs: Some(0),
        ..Default::default()
      })
      .is_err()
    );
    assert!(
      apply_patch(RuntimeConfigPatch {
        voter_replace_timeout_secs: Some(1),
        ..Default::default()
      })
      .is_err()
    );
  }
}

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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RuntimeConfig {
  /// Base node-announce interval in seconds, before cluster-size scaling
  /// (see `adaptive_announce_interval`).
  pub node_announce_interval_secs: u64,
  /// Seconds a member must stay unreachable before the membership guard
  /// acts on it (voter replaced / learner removed). Also the prune timeout
  /// for dead non-member nodes in the address book.
  pub voter_replace_timeout_secs: u64,
}

impl Default for RuntimeConfig {
  fn default() -> Self {
    Self {
      node_announce_interval_secs: crate::app::NODE_ANNOUNCE_INTERVAL.as_secs(),
      voter_replace_timeout_secs: 300,
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
}

/// Partial update: only the provided fields change.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RuntimeConfigPatch {
  pub node_announce_interval_secs: Option<u64>,
  pub voter_replace_timeout_secs: Option<u64>,
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

  RUNTIME_CONFIG.rcu(|current| {
    let mut next = (**current).clone();
    if let Some(secs) = patch.node_announce_interval_secs {
      next.node_announce_interval_secs = secs;
    }
    if let Some(secs) = patch.voter_replace_timeout_secs {
      next.voter_replace_timeout_secs = secs;
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

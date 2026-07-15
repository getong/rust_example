//! Unified cluster error type.
//!
//! One `ClusterError` enum replaces the former per-layer string wrappers
//! (`NetErr` in the swarm/transport layer, `BridgeErr` in the openraft
//! bridge), so error values compose across layers without re-wrapping and
//! callers match on one type. `anyhow::Error` remains the type for
//! application/setup paths (store opening, startup wiring) where errors are
//! only propagated and logged; anything that crosses the network or the
//! raft bridge converts into `ClusterError` via the `Other` variant.

use std::{error::Error, fmt, sync::Arc};

#[derive(Debug, Clone)]
pub enum ClusterError {
  /// libp2p transport / swarm failures: dial and connection errors, command
  /// channel closure, RPC timeouts, gossipsub publish failures.
  Network(String),
  /// openraft <-> libp2p bridge protocol violations, e.g. a response whose
  /// kind does not match the request.
  Bridge(String),
  /// Application-level failures funneled in from `anyhow` contexts. Shared
  /// behind an `Arc` because `ClusterError` is `Clone` (it travels through
  /// oneshot channels that fan out to several waiters) while `anyhow::Error`
  /// is not.
  Other(Arc<anyhow::Error>),
}

impl ClusterError {
  /// Build a `Network` error from anything displayable, mirroring the old
  /// `NetErr(format!(...))` construction.
  pub fn network(message: impl Into<String>) -> Self {
    Self::Network(message.into())
  }

  /// Build a `Bridge` error from anything displayable, mirroring the old
  /// `BridgeErr::new(...)` construction.
  pub fn bridge(message: impl Into<String>) -> Self {
    Self::Bridge(message.into())
  }
}

impl fmt::Display for ClusterError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    // The bare message, no variant prefix: these strings end up embedded in
    // openraft `Unreachable` errors and in log lines that existing tooling
    // already parses, exactly as the pre-unification wrappers printed them.
    match self {
      Self::Network(message) | Self::Bridge(message) => write!(f, "{message}"),
      Self::Other(error) => write!(f, "{error}"),
    }
  }
}

impl Error for ClusterError {}

impl From<anyhow::Error> for ClusterError {
  fn from(error: anyhow::Error) -> Self {
    Self::Other(Arc::new(error))
  }
}

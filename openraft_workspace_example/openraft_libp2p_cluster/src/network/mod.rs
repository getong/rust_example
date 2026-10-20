//! The cluster networking boundary.
//!
//! The crate contains two different concerns under this module:
//!
//! * [`openraft`] defines the Raft-facing protocol and adapter contracts.
//! * [`libp2p`] owns the concrete peer-to-peer transport and swarm.
//! * [`coordination`] contains transport-independent interpretation of Raft membership state used
//!   by startup and membership workflows.
//!
//! The flat modules below remain available for the existing internal call
//! sites. The façades make the intended dependency direction explicit for new
//! code and keep a future physical directory split backwards-compatible.

pub mod coordination;

/// OpenRaft-facing types. This namespace must not contain swarm lifecycle
/// code; it is the protocol/adapter side of the boundary.
pub mod openraft {
  pub use super::{openraft_dispatcher, openraft_sync, raft_bridge, rpc};
}

/// Concrete libp2p implementation. OpenRaft code should depend on the
/// `raft_bridge` traits rather than importing these implementation details.
pub mod libp2p {
  pub use super::{peer_guard, proto_codec, swarm, transport};
}

pub mod dispatcher;
pub mod openraft_dispatcher;
pub mod openraft_sync;
pub mod peer_guard;
pub mod proto_codec;
pub mod raft_bridge;
pub mod rpc;
pub mod swarm;
pub mod transport;

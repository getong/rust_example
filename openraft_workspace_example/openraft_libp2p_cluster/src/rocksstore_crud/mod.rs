//! Storage implementation for the v2 storage API: [`RaftLogStorage`] and
//! [`RaftStateMachine`] traits. Raft logs and state machine data are backed by
//! RocksDB, with snapshots using RocksDB's snapshot
//! mechanism for consistent point-in-time views.
#![allow(clippy::uninlined_format_args)]

pub mod log_store;
pub mod state_machine;

#[cfg(test)]
mod test;

use std::{convert::Infallible, fmt, io, path::Path, str::FromStr, sync::Arc};

use openraft::RaftTypeConfig;
use rocksdb::{ColumnFamilyDescriptor, DB, Options};
use serde::{Deserialize, Serialize};

use self::log_store::RocksLogStore;
pub use self::state_machine::RocksStateMachine;
use crate::types_kv;

#[derive(Serialize, Deserialize, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[serde(transparent)]
pub struct RocksNodeId(String);

impl RocksNodeId {
  pub fn new(id: impl Into<String>) -> Self {
    Self(id.into())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  /// The libp2p peer id this node id encodes. A node id IS the base58
  /// string form of the node's peer id, and this method (with the
  /// `From<PeerId>` impls below) is the single sanctioned conversion between
  /// the two — call sites must not hand-roll `PeerId::from_str` /
  /// `peer.to_string()` pairs.
  pub fn peer_id(&self) -> anyhow::Result<libp2p::PeerId> {
    self
      .0
      .parse()
      .map_err(|err| anyhow::anyhow!("node id {} is not a libp2p peer id: {err}", self.0))
  }
}

impl From<libp2p::PeerId> for RocksNodeId {
  fn from(peer: libp2p::PeerId) -> Self {
    Self(peer.to_string())
  }
}

impl From<&libp2p::PeerId> for RocksNodeId {
  fn from(peer: &libp2p::PeerId) -> Self {
    Self(peer.to_string())
  }
}

impl fmt::Display for RocksNodeId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl fmt::Debug for RocksNodeId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Debug::fmt(&self.0, f)
  }
}

impl From<String> for RocksNodeId {
  fn from(value: String) -> Self {
    Self(value)
  }
}

impl From<&str> for RocksNodeId {
  fn from(value: &str) -> Self {
    Self(value.to_string())
  }
}

impl From<u64> for RocksNodeId {
  fn from(value: u64) -> Self {
    Self(value.to_string())
  }
}

impl FromStr for RocksNodeId {
  type Err = Infallible;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(Self::from(s))
  }
}

openraft::declare_raft_types!(
    /// Declare the type configuration.
    pub TypeConfig:
        D = types_kv::Request,
        R = types_kv::Response,
        NodeId = RocksNodeId,
        AsyncRuntime = openraft::TokioRuntime,
);

/// Here you will set the types of request that will interact with the raft nodes.
/// For example the `Set` will be used to write data (key and value) to the raft database.
/// The `AddNode` will append a new node to the current existing shared list of nodes.
/// You will want to add any request that can write data in all nodes here.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RocksRequest {
  Set { key: String, value: String },
  Update { key: String, value: String },
  Delete { key: String },
}

impl fmt::Display for RocksRequest {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      RocksRequest::Set { key, value } => write!(f, "Set {{ key: {}, value: {} }}", key, value),
      RocksRequest::Update { key, value } => {
        write!(f, "Update {{ key: {}, value: {} }}", key, value)
      }
      RocksRequest::Delete { key } => write!(f, "Delete {{ key: {} }}", key),
    }
  }
}

/// Here you will define what type of answer you expect from reading the data of a node.
/// In this example it will return a optional value from a given key in
/// the `RocksRequest.Set`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RocksResponse {
  pub value: Option<String>,
}

impl From<types_kv::Response> for RocksResponse {
  fn from(response: types_kv::Response) -> Self {
    RocksResponse {
      value: response.value,
    }
  }
}

impl From<RocksRequest> for types_kv::Request {
  fn from(req: RocksRequest) -> Self {
    match req {
      RocksRequest::Set { key, value } | RocksRequest::Update { key, value } => {
        types_kv::Request::Set { key, value }
      }
      RocksRequest::Delete { key } => types_kv::Request::Delete { key },
    }
  }
}

/// Create a pair of `RocksLogStore` and `RocksStateMachine`.
pub async fn new<C, P: AsRef<Path>>(
  db_path: P,
) -> Result<(RocksLogStore<C>, RocksStateMachine), io::Error>
where
  C: RaftTypeConfig,
{
  let mut db_opts = Options::default();
  db_opts.create_missing_column_families(true);
  db_opts.create_if_missing(true);

  let meta = ColumnFamilyDescriptor::new("meta", Options::default());
  let sm_meta = ColumnFamilyDescriptor::new("sm_meta", Options::default());
  let sm_data = ColumnFamilyDescriptor::new("sm_data", Options::default());
  let logs = ColumnFamilyDescriptor::new("logs", Options::default());

  let db_path = db_path.as_ref();
  let snapshot_dir = db_path.join("snapshots");

  let db = DB::open_cf_descriptors(&db_opts, db_path, vec![meta, sm_meta, sm_data, logs])
    .map_err(io::Error::other)?;

  let db = Arc::new(db);
  Ok((
    RocksLogStore::new(db.clone())?,
    RocksStateMachine::new(db, snapshot_dir).await?,
  ))
}

#[cfg(test)]
mod node_id_tests {
  use super::RocksNodeId;

  #[test]
  fn node_id_and_peer_id_roundtrip() {
    let peer = libp2p::PeerId::random();
    let node_id = RocksNodeId::from(peer);
    assert_eq!(node_id.as_str(), peer.to_string());
    assert_eq!(node_id.peer_id().expect("parse peer id"), peer);
    assert_eq!(RocksNodeId::from(&peer), node_id);
  }

  #[test]
  fn non_peer_node_id_fails_conversion() {
    assert!(RocksNodeId::new("not-a-peer-id").peer_id().is_err());
  }
}

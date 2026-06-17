//! This rocks-db backed storage implement the v2 storage API: [`RaftLogStorage`] and
//! [`RaftStateMachine`] traits. The state machine stores all data directly in RocksDB,
//! providing full persistence. Log entries are applied directly to disk, and snapshots
//! use RocksDB's snapshot mechanism for consistent point-in-time views.
#![allow(clippy::uninlined_format_args)]

pub mod log_store;
pub mod state_machine;

use std::{convert::Infallible, fmt, io, path::Path, str::FromStr, sync::Arc};

use openraft::RaftTypeConfig;
use rocksdb::{ColumnFamilyDescriptor, DB, Options};
use serde::{Deserialize, Serialize};

use self::log_store::RocksLogStore;
pub use self::state_machine::RocksStateMachine;

#[derive(Serialize, Deserialize, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[serde(transparent)]
pub struct RocksNodeId(String);

impl RocksNodeId {
  pub fn new(id: impl Into<String>) -> Self {
    Self(id.into())
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
        D = crate::kv_types::Request,
        R = crate::kv_types::Response,
        NodeId = RocksNodeId,
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

impl From<crate::kv_types::Request> for RocksRequest {
  fn from(request: crate::kv_types::Request) -> Self {
    match request {
      crate::kv_types::Request::Set { key, value } => RocksRequest::Set { key, value },
    }
  }
}

impl From<RocksRequest> for crate::kv_types::Request {
  fn from(req: RocksRequest) -> Self {
    match req {
      RocksRequest::Set { key, value } | RocksRequest::Update { key, value } => {
        crate::kv_types::Request::Set { key, value }
      }
      RocksRequest::Delete { key } => crate::kv_types::Request::Set {
        key,
        value: String::new(),
      },
    }
  }
}

/// Create a pair of `RocksLogStore` and `RocksStateMachine` that are backed by a same rocks db
/// instance.
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
    RocksLogStore::new(db.clone()),
    RocksStateMachine::new(db, snapshot_dir).await?,
  ))
}

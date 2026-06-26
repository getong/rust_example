//! RocksDB-backed OpenRaft storage.
#![allow(clippy::uninlined_format_args)]

pub mod log_store;
pub mod state_machine;

#[cfg(test)]
mod test;

use std::{io, path::Path, sync::Arc};

use openraft::RaftTypeConfig;
use rocksdb::{ColumnFamilyDescriptor, DB, Options};

use self::log_store::RocksLogStore;
pub use self::state_machine::RocksStateMachine;
use crate::types_kv::{QueueCommand, QueueResponse};

openraft::declare_raft_types!(
    /// Type configuration for the replicated task queue.
    pub TypeConfig:
        D = QueueCommand,
        R = QueueResponse,
);

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

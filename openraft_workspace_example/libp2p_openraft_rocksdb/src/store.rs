//! RocksDB-backed storage.

use std::path::Path;

use anyhow::Context;
use openraft_rocksstore::{RocksStateMachine, TypeConfig, log_store::RocksLogStore};

pub type LogStore = RocksLogStore<TypeConfig>;
pub type StateMachineStore = RocksStateMachine;

pub async fn open_store<P: AsRef<Path>>(
  db_dir: P,
) -> anyhow::Result<(LogStore, StateMachineStore)> {
  openraft_rocksstore::new::<TypeConfig, _>(db_dir)
    .await
    .context("open rocksdb store")
}

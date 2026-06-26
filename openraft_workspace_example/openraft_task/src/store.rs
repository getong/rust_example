//! Helpers for opening RocksDB-backed OpenRaft stores.

use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::rocksstore_crud::{RocksStateMachine, TypeConfig, log_store::RocksLogStore};

pub type LogStore = RocksLogStore<TypeConfig>;
pub type StateMachineStore = RocksStateMachine;

pub async fn open_store<P: AsRef<Path>>(
  db_dir: P,
) -> anyhow::Result<(LogStore, StateMachineStore)> {
  crate::rocksstore_crud::new::<TypeConfig, _>(db_dir)
    .await
    .context("open rocksdb store")
}

pub fn node_db_dir(base_dir: &Path, node_id: u64) -> PathBuf {
  base_dir.join(format!("node-{node_id}"))
}

//! RocksDB-backed storage.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use anyhow::Context;
use openraft_rocksstore::{RocksStateMachine, TypeConfig, log_store::RocksLogStore};
use tokio::sync::RwLock;

pub type LogStore = RocksLogStore<TypeConfig>;
pub type StateMachineStore = RocksStateMachine;
pub type KvData = Arc<RwLock<BTreeMap<String, String>>>;

pub async fn open_store<P: AsRef<Path>>(
  db_dir: P,
) -> anyhow::Result<(LogStore, StateMachineStore)> {
  openraft_rocksstore::new::<TypeConfig, _>(db_dir)
    .await
    .context("open rocksdb store")
}

pub fn kv_data(state_machine: &StateMachineStore) -> KvData {
  state_machine.kvs()
}

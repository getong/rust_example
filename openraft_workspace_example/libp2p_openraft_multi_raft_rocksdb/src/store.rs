//! RocksDB-backed storage.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use anyhow::Context;
use openraft::ReadPolicy;
use openraft_rocksstore_crud::{RocksStateMachine, TypeConfig, log_store::RocksLogStore};
use tokio::sync::RwLock;

use crate::typ::{LinearizableReadError, Raft, RaftError};

pub type LogStore = RocksLogStore<TypeConfig>;
pub type StateMachineStore = RocksStateMachine;
pub type KvData = Arc<RwLock<BTreeMap<String, String>>>;

pub async fn open_store<P: AsRef<Path>>(
  db_dir: P,
) -> anyhow::Result<(LogStore, StateMachineStore)> {
  openraft_rocksstore_crud::new::<TypeConfig, _>(db_dir)
    .await
    .context("open rocksdb store")
}

pub fn kv_data(state_machine: &StateMachineStore) -> KvData {
  state_machine.kvs()
}

pub async fn ensure_linearizable_read(raft: &Raft) -> Result<(), RaftError<LinearizableReadError>> {
  let linearizer = raft.get_read_linearizer(ReadPolicy::ReadIndex).await?;
  linearizer
    .await_ready(raft)
    .await
    .map(|_| ())
    .map_err(RaftError::Fatal)
}

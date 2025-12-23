//! RocksDB-backed storage.

use std::{path::Path, sync::Arc};

use anyhow::{Context, anyhow};
use openraft_rocksstore::{RocksStateMachine, TypeConfig, log_store::RocksLogStore};
use rocksdb::{DB, Options};
use tokio::task::spawn_blocking;

pub type LogStore = RocksLogStore<TypeConfig>;
pub type StateMachineStore = RocksStateMachine;

#[derive(Clone)]
pub struct KvStoreReader {
  db: Arc<DB>,
}

pub async fn open_store<P: AsRef<Path>>(
  db_dir: P,
) -> anyhow::Result<(LogStore, StateMachineStore)> {
  openraft_rocksstore::new::<TypeConfig, _>(db_dir)
    .await
    .context("open rocksdb store")
}

pub fn open_kv_reader<P: AsRef<Path>>(db_dir: P) -> anyhow::Result<KvStoreReader> {
  let mut db_opts = Options::default();
  db_opts.create_if_missing(false);
  db_opts.create_missing_column_families(false);

  let cfs = ["meta", "sm_meta", "sm_data", "logs"];
  let db = DB::open_cf_for_read_only(&db_opts, db_dir, cfs, false)
    .map_err(|err| anyhow!("open kv reader db: {err}"))?;
  Ok(KvStoreReader { db: Arc::new(db) })
}

impl KvStoreReader {
  pub async fn get(&self, key: String) -> anyhow::Result<Option<String>> {
    let db = self.db.clone();
    spawn_blocking(move || {
      let cf = db
        .cf_handle("sm_data")
        .ok_or_else(|| anyhow!("column family sm_data not found"))?;
      let value = db.get_cf(cf, key.as_bytes())?;
      Ok(value.map(|bytes| String::from_utf8_lossy(&bytes).to_string()))
    })
    .await
    .map_err(|err| anyhow!("kv reader task error: {err}"))?
  }

  pub async fn exists(&self, key: String) -> anyhow::Result<bool> {
    let db = self.db.clone();
    spawn_blocking(move || {
      let cf = db
        .cf_handle("sm_data")
        .ok_or_else(|| anyhow!("column family sm_data not found"))?;
      Ok(db.get_cf(cf, key.as_bytes())?.is_some())
    })
    .await
    .map_err(|err| anyhow!("kv reader task error: {err}"))?
  }
}

//! RocksDB-backed storage.

use std::{
  ffi::OsString,
  fs,
  path::{Path, PathBuf},
  sync::Arc,
};

use anyhow::Context;
use openraft::{ReadPolicy, type_config::TypeConfigExt};
use openraft_rocksstore_crud::{RocksStateMachine, TypeConfig, log_store::RocksLogStore};
use rocksdb::{ColumnFamilyRef, DB, Options};

use crate::typ::{LinearizableReadError, Raft, RaftError, StoredMembership};

pub type LogStore = RocksLogStore<TypeConfig>;
pub type StateMachineStore = RocksStateMachine;

const SM_DATA_CF: &str = "sm_data";
const STORE_CFS: [&str; 4] = ["meta", "sm_meta", SM_DATA_CF, "logs"];

#[derive(Debug, Clone)]
pub struct KvData {
  db: Arc<DB>,
}

impl KvData {
  pub fn open<P: AsRef<Path>>(primary_path: P) -> anyhow::Result<Self> {
    let primary_path = primary_path.as_ref();
    let secondary_path = secondary_db_dir(primary_path);
    if let Some(parent) = secondary_path.parent() {
      fs::create_dir_all(parent)
        .with_context(|| format!("create rocksdb secondary parent: {}", parent.display()))?;
    }
    fs::create_dir_all(&secondary_path)
      .with_context(|| format!("create rocksdb secondary dir: {}", secondary_path.display()))?;

    let mut opts = Options::default();
    opts.set_max_open_files(-1);

    let db = DB::open_cf_as_secondary(&opts, primary_path, &secondary_path, STORE_CFS)
      .with_context(|| {
        format!(
          "open rocksdb secondary reader: primary={}, secondary={}",
          primary_path.display(),
          secondary_path.display()
        )
      })?;

    let kv_data = Self { db: Arc::new(db) };
    kv_data.catch_up()?;
    Ok(kv_data)
  }

  pub async fn get(&self, key: &str) -> anyhow::Result<Option<String>> {
    let db = self.db.clone();
    let key = key.to_string();
    TypeConfig::spawn_blocking(move || {
      catch_up(&db)?;
      let cf = sm_data_cf(&db)?;
      db.get_cf(&cf, key.as_bytes())
        .context("read rocksdb kv value")?
        .map(|value| decode_utf8(value.as_ref(), "value"))
        .transpose()
    })
    .await
    .context("join rocksdb kv get task")?
  }

  pub async fn contains_key(&self, key: &str) -> anyhow::Result<bool> {
    self.get(key).await.map(|value| value.is_some())
  }

  pub async fn entries(&self) -> anyhow::Result<Vec<(String, String)>> {
    let db = self.db.clone();
    TypeConfig::spawn_blocking(move || {
      catch_up(&db)?;
      let cf = sm_data_cf(&db)?;
      let iter = db.iterator_cf(&cf, rocksdb::IteratorMode::Start);
      let mut entries = Vec::new();
      for item in iter {
        let (key, value) = item.context("iterate rocksdb kv data")?;
        entries.push((
          decode_utf8(key.as_ref(), "key")?,
          decode_utf8(value.as_ref(), "value")?,
        ));
      }
      Ok(entries)
    })
    .await
    .context("join rocksdb kv entries task")?
  }

  fn catch_up(&self) -> anyhow::Result<()> {
    catch_up(&self.db)
  }
}

fn sm_data_cf(db: &DB) -> anyhow::Result<ColumnFamilyRef<'_>> {
  db.cf_handle(SM_DATA_CF)
    .ok_or_else(|| anyhow::anyhow!("column family `{SM_DATA_CF}` not found"))
}

fn catch_up(db: &DB) -> anyhow::Result<()> {
  db.try_catch_up_with_primary()
    .context("catch up rocksdb secondary reader with primary")
}

fn decode_utf8(bytes: &[u8], what: &str) -> anyhow::Result<String> {
  String::from_utf8(bytes.to_vec()).with_context(|| format!("decode rocksdb kv {what} as utf-8"))
}

pub async fn open_store<P: AsRef<Path>>(
  db_dir: P,
) -> anyhow::Result<(LogStore, StateMachineStore)> {
  openraft_rocksstore_crud::new::<TypeConfig, _>(db_dir)
    .await
    .context("open rocksdb store")
}

pub fn group_db_dir(base_dir: &Path, group_id: &str) -> PathBuf {
  base_dir.join(group_id)
}

pub fn read_persisted_membership_for_group(
  db_dir: &Path,
  group_id: &str,
) -> anyhow::Result<Option<StoredMembership>> {
  let db_path = group_db_dir(db_dir, group_id);
  if !db_path.join("CURRENT").exists() {
    return Ok(None);
  }

  let mut opts = Options::default();
  opts.set_max_open_files(-1);

  let db = DB::open_cf_for_read_only(&opts, &db_path, STORE_CFS, false)
    .with_context(|| format!("open rocksdb read-only: {}", db_path.display()))?;
  let cf = db
    .cf_handle("sm_meta")
    .ok_or_else(|| anyhow::anyhow!("column family `sm_meta` not found"))?;
  let Some(bytes) = db
    .get_cf(&cf, "last_membership")
    .context("read persisted openraft membership")?
  else {
    return Ok(None);
  };

  let membership =
    serde_json::from_slice(&bytes).context("decode persisted openraft membership")?;
  Ok(Some(membership))
}

pub fn remove_group_store(db_dir: &Path, group_id: &str) -> anyhow::Result<()> {
  let db_path = group_db_dir(db_dir, group_id);
  let secondary_path = secondary_db_dir(&db_path);

  if secondary_path.exists() {
    fs::remove_dir_all(&secondary_path)
      .with_context(|| format!("remove rocksdb secondary dir: {}", secondary_path.display()))?;
  }
  if db_path.exists() {
    fs::remove_dir_all(&db_path)
      .with_context(|| format!("remove rocksdb group dir: {}", db_path.display()))?;
  }

  Ok(())
}

pub async fn open_store_for_group<P: AsRef<Path>>(
  db_dir: P,
  group_id: &str,
) -> anyhow::Result<(LogStore, StateMachineStore, KvData)> {
  let db_path = group_db_dir(db_dir.as_ref(), group_id);
  let (log_store, state_machine) = open_store(&db_path).await?;
  let kv_data = KvData::open(&db_path)?;
  Ok((log_store, state_machine, kv_data))
}

pub async fn ensure_linearizable_read(raft: &Raft) -> Result<(), RaftError<LinearizableReadError>> {
  let linearizer = raft.get_read_linearizer(ReadPolicy::ReadIndex).await?;
  linearizer
    .await_ready(raft)
    .await
    .map(|_| ())
    .map_err(RaftError::Fatal)
}

fn secondary_db_dir(primary_path: &Path) -> PathBuf {
  let mut secondary_name = primary_path
    .file_name()
    .map(OsString::from)
    .unwrap_or_else(|| OsString::from("rocksdb"));
  secondary_name.push(".secondary");
  primary_path.with_file_name(secondary_name)
}

#[cfg(test)]
mod tests {
  use rocksdb::{ColumnFamilyDescriptor, Options};

  use super::*;

  #[tokio::test]
  async fn kv_data_reads_from_rocksdb_secondary() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let primary_path = temp.path().join("primary");

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let cfs = STORE_CFS
      .into_iter()
      .map(|name| ColumnFamilyDescriptor::new(name, Options::default()));
    let db = DB::open_cf_descriptors(&opts, &primary_path, cfs).expect("open primary");
    let cf = db.cf_handle(SM_DATA_CF).expect("sm_data cf");
    db.put_cf(&cf, b"alpha", b"one").expect("write alpha");

    let kv_data = KvData::open(&primary_path).expect("open kv data");
    assert_eq!(
      kv_data.get("alpha").await.expect("get alpha"),
      Some("one".to_string())
    );

    db.put_cf(&cf, b"alpha", b"two").expect("write alpha again");
    assert_eq!(
      kv_data.get("alpha").await.expect("get alpha again"),
      Some("two".to_string())
    );

    db.put_cf(&cf, b"beta", b"three").expect("write beta");
    let entries = kv_data.entries().await.expect("entries");
    assert_eq!(
      entries,
      vec![
        ("alpha".to_string(), "two".to_string()),
        ("beta".to_string(), "three".to_string())
      ]
    );
  }
}

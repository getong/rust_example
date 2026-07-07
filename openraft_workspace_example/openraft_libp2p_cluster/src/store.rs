//! RocksDB-backed Raft logs and state machine.

use std::{
  ffi::OsString,
  fs,
  path::{Path, PathBuf},
  sync::Arc,
};

use anyhow::Context;
use openraft::{
  RaftLogReader, ReadPolicy,
  alias::LogIdOf,
  entry::RaftEntry,
  storage::{RaftLogStorage, RaftStateMachine},
  type_config::TypeConfigExt,
};
use rocksdb::{ColumnFamilyRef, DB, Options};

use crate::{
  rocksstore_crud::{
    RocksStateMachine, TypeConfig, log_store::RocksLogStore,
    state_machine::read_latest_snapshot_meta,
  },
  typ::{LinearizableReadError, Raft, RaftError, StoredMembership},
};

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

  pub async fn entries_with_prefix(&self, prefix: String) -> anyhow::Result<Vec<(String, String)>> {
    let db = self.db.clone();
    TypeConfig::spawn_blocking(move || {
      catch_up(&db)?;
      let cf = sm_data_cf(&db)?;
      let iter = db.iterator_cf(
        &cf,
        rocksdb::IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward),
      );
      let mut entries = Vec::new();
      for item in iter {
        let (key, value) = item.context("iterate rocksdb kv data")?;
        if !key.starts_with(prefix.as_bytes()) {
          break;
        }
        entries.push((
          decode_utf8(key.as_ref(), "key")?,
          decode_utf8(value.as_ref(), "value")?,
        ));
      }
      Ok(entries)
    })
    .await
    .context("join rocksdb kv entries_with_prefix task")?
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
  crate::rocksstore_crud::new::<TypeConfig, _>(db_dir)
    .await
    .context("open rocksdb log and state store")
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
    return read_persisted_snapshot_membership(&db_path);
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
    return read_persisted_snapshot_membership(&db_path);
  };

  let membership = sonic_rs::from_slice(&bytes).context("decode persisted openraft membership")?;
  Ok(Some(membership))
}

fn read_persisted_snapshot_membership(db_path: &Path) -> anyhow::Result<Option<StoredMembership>> {
  let snapshot_dir = db_path.join("snapshots");
  if !snapshot_dir.exists() {
    return Ok(None);
  }

  let Some(meta) = read_latest_snapshot_meta(&snapshot_dir).with_context(|| {
    format!(
      "read persisted openraft snapshot: {}",
      snapshot_dir.display()
    )
  })?
  else {
    return Ok(None);
  };

  Ok(Some(meta.last_membership))
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
  let (mut log_store, mut state_machine) = open_store(&db_path).await?;
  verify_openraft_store_integrity(group_id, &mut log_store, &mut state_machine).await?;
  let kv_data = KvData::open(&db_path)?;
  Ok((log_store, state_machine, kv_data))
}

pub async fn verify_openraft_store_integrity(
  group_id: &str,
  log_store: &mut LogStore,
  state_machine: &mut StateMachineStore,
) -> anyhow::Result<()> {
  let vote = log_store
    .read_vote()
    .await
    .with_context(|| format!("verify group {group_id}: read persisted vote"))?;
  let committed = log_store
    .read_committed()
    .await
    .with_context(|| format!("verify group {group_id}: read persisted committed log id"))?;
  let log_state = log_store
    .get_log_state()
    .await
    .with_context(|| format!("verify group {group_id}: read persisted log state"))?;
  let (last_applied, last_membership) = state_machine
    .applied_state()
    .await
    .with_context(|| format!("verify group {group_id}: read persisted state machine metadata"))?;
  let current_snapshot = state_machine
    .get_current_snapshot()
    .await
    .with_context(|| format!("verify group {group_id}: read persisted snapshot"))?;
  let snapshot_last_log_id = current_snapshot
    .as_ref()
    .and_then(|snapshot| snapshot.meta.last_log_id.clone());

  verify_log_id_order(
    group_id,
    "last_purged_log_id",
    log_state.last_purged_log_id.as_ref(),
    "last_log_id",
    log_state.last_log_id.as_ref(),
  )?;

  let durable_tip = max_log_id(
    max_log_id(log_state.last_log_id.clone(), last_applied.clone()),
    snapshot_last_log_id.clone(),
  );
  verify_optional_log_id_not_after(
    group_id,
    "committed",
    committed.as_ref(),
    "durable_tip",
    durable_tip.as_ref(),
  )?;
  verify_optional_log_id_not_after(
    group_id,
    "last_applied",
    last_applied.as_ref(),
    "durable_tip",
    durable_tip.as_ref(),
  )?;

  verify_log_entries(
    group_id,
    log_store,
    log_state.last_purged_log_id,
    log_state.last_log_id,
  )
  .await?;

  tracing::info!(
    group = group_id,
    ?vote,
    ?committed,
    ?last_applied,
    ?snapshot_last_log_id,
    membership_nodes = last_membership.membership().nodes().count(),
    "verified openraft store after rocksdb wal recovery"
  );

  Ok(())
}

async fn verify_log_entries(
  group_id: &str,
  log_store: &mut LogStore,
  last_purged_log_id: Option<LogIdOf<TypeConfig>>,
  last_log_id: Option<LogIdOf<TypeConfig>>,
) -> anyhow::Result<()> {
  let Some(last_log_id) = last_log_id else {
    return Ok(());
  };

  let start = last_purged_log_id
    .as_ref()
    .and_then(|log_id| log_id.index().checked_add(1))
    .unwrap_or(0);

  if last_log_id.index() < start {
    return Ok(());
  }

  let entries = log_store
    .try_get_log_entries(start ..= last_log_id.index())
    .await
    .with_context(|| {
      format!(
        "verify group {group_id}: read log entries {}..={}",
        start,
        last_log_id.index()
      )
    })?;

  let mut entries = entries.into_iter();
  let Some(first_entry) = entries.next() else {
    return Err(anyhow::anyhow!(
      "verify group {group_id}: log state has last_log_id {last_log_id:?} but no log entries were \
       readable"
    ));
  };

  let mut expected_index = last_purged_log_id
    .as_ref()
    .and_then(|log_id| log_id.index().checked_add(1))
    .unwrap_or_else(|| first_entry.index());
  let mut previous_log_id: Option<LogIdOf<TypeConfig>> = None;
  for entry in std::iter::once(first_entry).chain(entries) {
    let entry_index = entry.index();
    if entry_index != expected_index {
      return Err(anyhow::anyhow!(
        "verify group {group_id}: missing or out-of-order log entry: expected index \
         {expected_index}, got {entry_index}"
      ));
    }

    let entry_log_id = entry.log_id();
    if let Some(previous) = previous_log_id.as_ref() {
      if previous >= &entry_log_id {
        return Err(anyhow::anyhow!(
          "verify group {group_id}: non-increasing log id: previous={previous:?}, \
           current={entry_log_id:?}"
        ));
      }
    }

    previous_log_id = Some(entry_log_id);
    expected_index = expected_index
      .checked_add(1)
      .ok_or_else(|| anyhow::anyhow!("verify group {group_id}: log index overflow"))?;
  }

  if expected_index <= last_log_id.index() {
    return Err(anyhow::anyhow!(
      "verify group {group_id}: missing log entries through last_log_id {last_log_id:?}"
    ));
  }

  if previous_log_id.as_ref() != Some(&last_log_id) {
    return Err(anyhow::anyhow!(
      "verify group {group_id}: last persisted entry {previous_log_id:?} does not match log state \
       {last_log_id:?}"
    ));
  }

  Ok(())
}

fn verify_log_id_order(
  group_id: &str,
  lower_name: &str,
  lower: Option<&LogIdOf<TypeConfig>>,
  upper_name: &str,
  upper: Option<&LogIdOf<TypeConfig>>,
) -> anyhow::Result<()> {
  if let (Some(lower), Some(upper)) = (lower, upper) {
    if lower > upper {
      return Err(anyhow::anyhow!(
        "verify group {group_id}: {lower_name} {lower:?} is after {upper_name} {upper:?}"
      ));
    }
  }

  Ok(())
}

fn verify_optional_log_id_not_after(
  group_id: &str,
  value_name: &str,
  value: Option<&LogIdOf<TypeConfig>>,
  tip_name: &str,
  tip: Option<&LogIdOf<TypeConfig>>,
) -> anyhow::Result<()> {
  match (value, tip) {
    (Some(value), Some(tip)) if value > tip => Err(anyhow::anyhow!(
      "verify group {group_id}: {value_name} {value:?} is after {tip_name} {tip:?}"
    )),
    (Some(value), None) => Err(anyhow::anyhow!(
      "verify group {group_id}: {value_name} {value:?} exists but {tip_name} is empty"
    )),
    _ => Ok(()),
  }
}

fn max_log_id(
  left: Option<LogIdOf<TypeConfig>>,
  right: Option<LogIdOf<TypeConfig>>,
) -> Option<LogIdOf<TypeConfig>> {
  match (left, right) {
    (Some(left), Some(right)) => Some(std::cmp::max(left, right)),
    (Some(log_id), None) | (None, Some(log_id)) => Some(log_id),
    (None, None) => None,
  }
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
  use std::{collections::BTreeSet, fs, path::Path};

  use openraft::{
    EntryPayload, Membership, RaftTypeConfig,
    alias::LogIdOf,
    entry::RaftEntry,
    storage::{RaftSnapshotBuilder, RaftStateMachine},
    vote::RaftLeaderIdExt,
  };
  use rocksdb::{ColumnFamilyDescriptor, Options};

  use super::*;
  use crate::NodeId;

  fn log_id(node_id: &NodeId) -> LogIdOf<TypeConfig> {
    LogIdOf::<TypeConfig>::new(
      <TypeConfig as RaftTypeConfig>::LeaderId::new_committed(1, node_id.clone()),
      1,
    )
  }

  fn membership_entry(node_id: NodeId) -> <TypeConfig as RaftTypeConfig>::Entry {
    let voters = BTreeSet::from([node_id.clone()]);
    <TypeConfig as RaftTypeConfig>::Entry::new(
      log_id(&node_id),
      EntryPayload::Membership(Membership::new_with_defaults(vec![voters], [])),
    )
  }

  fn remove_non_snapshot_files(path: &Path) {
    for entry in fs::read_dir(path).expect("read db dir") {
      let entry = entry.expect("db dir entry");
      if entry.file_name().to_string_lossy() == "snapshots" {
        continue;
      }

      let path = entry.path();
      if path.is_dir() {
        fs::remove_dir_all(&path).expect("remove db child dir");
      } else {
        fs::remove_file(&path).expect("remove db child file");
      }
    }
  }

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

  #[tokio::test]
  async fn read_persisted_membership_falls_back_to_snapshot() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let group_id = "users";
    let node_id = NodeId::from("node-a");
    let db_path = group_db_dir(temp.path(), group_id);

    {
      let (_log_store, mut state_machine, _kv_data) = open_store_for_group(temp.path(), group_id)
        .await
        .expect("open group store");
      state_machine
        .apply(futures::stream::iter([Ok((
          membership_entry(node_id.clone()),
          None,
        ))]))
        .await
        .expect("apply membership");
      state_machine
        .build_snapshot()
        .await
        .expect("build membership snapshot");
    }

    remove_non_snapshot_files(&db_path);

    let membership = read_persisted_membership_for_group(temp.path(), group_id)
      .expect("read persisted membership")
      .expect("membership from snapshot");
    assert!(membership.membership().get_node(&node_id).is_some());
  }
}

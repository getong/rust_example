//! RocksDB-backed state machine implementation.

use std::{
  collections::{BTreeMap, HashMap},
  fmt::Debug,
  fs,
  fs::File,
  io,
  io::{Cursor, Write},
  path::{Path, PathBuf},
  sync::Arc,
};

use futures::{Stream, TryStreamExt};
use openraft::{
  EntryPayload, OptionalSend, RaftSnapshotBuilder, StorageError,
  alias::{LogIdOf, SnapshotMetaOf, SnapshotOf, StoredMembershipOf},
  entry::RaftEntry,
  storage::{EntryResponder, RaftStateMachine},
  type_config::TypeConfigExt,
};
use rocksdb::{DB, WriteOptions};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::TypeConfig;
use crate::{tasks, types_kv};

const SM_META_CF: &str = "sm_meta";
const SM_DATA_CF: &str = "sm_data";
const LAST_APPLIED_LOG_KEY: &str = "last_applied_log";
const LAST_MEMBERSHIP_KEY: &str = "last_membership";
const SNAPSHOT_TMP_SUFFIX: &str = ".tmp";

/// State machine backed by RocksDB for full persistence.
/// All application data is stored directly in the `sm_data` column family.
/// Snapshots are persisted to the `snapshot_dir` directory.
#[derive(Debug, Clone)]
pub struct RocksStateMachine {
  db: Arc<DB>,
  snapshot_dir: PathBuf,
  data: Arc<RwLock<BTreeMap<String, String>>>,
}

#[derive(Debug)]
enum DataChange {
  Set { key: String, value: String },
  Delete { key: String },
}

impl RocksStateMachine {
  pub(crate) async fn new(
    db: Arc<DB>,
    snapshot_dir: PathBuf,
  ) -> Result<RocksStateMachine, io::Error> {
    // Validate column families exist at construction time
    db.cf_handle(SM_META_CF)
      .ok_or_else(|| io::Error::other(format!("column family `{SM_META_CF}` not found")))?;
    db.cf_handle(SM_DATA_CF)
      .ok_or_else(|| io::Error::other(format!("column family `{SM_DATA_CF}` not found")))?;

    // Create snapshot directory if it doesn't exist
    fs::create_dir_all(&snapshot_dir)?;
    recover_from_latest_snapshot_if_newer(db.clone(), &snapshot_dir).await?;

    let data = TypeConfig::spawn_blocking({
      let db = db.clone();
      move || -> Result<BTreeMap<String, String>, io::Error> {
        let cf_data = db
          .cf_handle(SM_DATA_CF)
          .ok_or_else(|| io::Error::other(format!("column family `{SM_DATA_CF}` not found")))?;
        let iter = db.iterator_cf(cf_data, rocksdb::IteratorMode::Start);
        let mut map = BTreeMap::new();
        for item in iter {
          let (key, value) = item.map_err(|e| io::Error::other(e.to_string()))?;
          let key = decode_utf8(key.to_vec(), "snapshot key")?;
          let value = decode_utf8(value.to_vec(), "snapshot value")?;
          map.insert(key, value);
        }
        Ok(map)
      }
    })
    .await??;

    Ok(Self {
      db,
      snapshot_dir,
      data: Arc::new(RwLock::new(data)),
    })
  }

  fn cf_sm_meta(&self) -> &rocksdb::ColumnFamily {
    self
      .db
      .cf_handle(SM_META_CF)
      .expect("column family `sm_meta` not found")
  }

  pub fn kvs(&self) -> Arc<RwLock<BTreeMap<String, String>>> {
    self.data.clone()
  }

  #[allow(clippy::type_complexity)]
  fn get_meta(
    &self,
  ) -> Result<(Option<LogIdOf<TypeConfig>>, StoredMembershipOf<TypeConfig>), StorageError<TypeConfig>>
  {
    let cf = self.cf_sm_meta();

    let last_applied_log = self
      .db
      .get_cf(cf, LAST_APPLIED_LOG_KEY)
      .map_err(|e| StorageError::read(TypeConfig::err_from_error(&e)))?
      .map(|bytes| deserialize(&bytes))
      .transpose()?;

    let last_membership = self
      .db
      .get_cf(cf, LAST_MEMBERSHIP_KEY)
      .map_err(|e| StorageError::read(TypeConfig::err_from_error(&e)))?
      .map(|bytes| deserialize(&bytes))
      .transpose()?
      .unwrap_or_default();

    Ok((last_applied_log, last_membership))
  }
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use openraft::{
    EntryPayload, RaftTypeConfig,
    alias::LogIdOf,
    entry::RaftEntry,
    storage::{RaftSnapshotBuilder, RaftStateMachine},
    vote::RaftLeaderIdExt,
  };

  use super::*;
  use crate::rocksstore_crud;

  fn log_id(index: u64) -> LogIdOf<TypeConfig> {
    LogIdOf::<TypeConfig>::new(
      <TypeConfig as RaftTypeConfig>::LeaderId::new_committed(
        1,
        rocksstore_crud::RocksNodeId::new("node-a"),
      ),
      index,
    )
  }

  fn set_entry(
    index: u64,
    key: impl Into<String>,
    value: impl Into<String>,
  ) -> <TypeConfig as RaftTypeConfig>::Entry {
    <TypeConfig as RaftTypeConfig>::Entry::new(
      log_id(index),
      EntryPayload::Normal(types_kv::Request::Set {
        key: key.into(),
        value: value.into(),
      }),
    )
  }

  async fn apply_entries(
    sm: &mut RocksStateMachine,
    entries: Vec<<TypeConfig as RaftTypeConfig>::Entry>,
  ) {
    sm.apply(futures::stream::iter(
      entries.into_iter().map(|entry| Ok((entry, None))),
    ))
    .await
    .expect("apply entries");
  }

  fn remove_non_snapshot_files(path: &Path) {
    for entry in fs::read_dir(path).expect("read db dir") {
      let entry = entry.expect("db dir entry");
      if entry.file_name() == "snapshots" {
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
  async fn rocks_state_machine_reopens_rocksdb_data_and_meta() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let expected_log_id = log_id(1);

    {
      let (_log_store, mut sm) = rocksstore_crud::new::<TypeConfig, _>(temp.path())
        .await
        .expect("open store");
      apply_entries(&mut sm, vec![set_entry(1, "alpha", "one")]).await;
      let (last_applied, _) = sm.applied_state().await.expect("applied state");
      assert_eq!(Some(expected_log_id.clone()), last_applied);
    }

    {
      let (_log_store, mut sm) = rocksstore_crud::new::<TypeConfig, _>(temp.path())
        .await
        .expect("reopen store");
      let (last_applied, _) = sm.applied_state().await.expect("reopened applied state");
      assert_eq!(Some(expected_log_id), last_applied);
      assert_eq!(
        Some("one".to_string()),
        sm.kvs().read().await.get("alpha").cloned()
      );
    }
  }

  #[tokio::test]
  async fn rocks_state_machine_restores_from_snapshot_when_rocksdb_is_missing() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let expected_log_id = log_id(1);

    {
      let (_log_store, mut sm) = rocksstore_crud::new::<TypeConfig, _>(temp.path())
        .await
        .expect("open store");
      apply_entries(&mut sm, vec![set_entry(1, "alpha", "one")]).await;
      sm.build_snapshot().await.expect("build snapshot");
    }

    remove_non_snapshot_files(temp.path());

    {
      let (_log_store, mut sm) = rocksstore_crud::new::<TypeConfig, _>(temp.path())
        .await
        .expect("reopen store from snapshot");
      let (last_applied, _) = sm.applied_state().await.expect("restored applied state");
      assert_eq!(Some(expected_log_id), last_applied);
      assert_eq!(
        Some("one".to_string()),
        sm.kvs().read().await.get("alpha").cloned()
      );
    }
  }
}

fn deserialize<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, StorageError<TypeConfig>> {
  sonic_rs::from_slice(bytes).map_err(|e| StorageError::read(TypeConfig::err_from_error(&e)))
}

fn serialize_io<T: Serialize>(value: &T) -> Result<Vec<u8>, io::Error> {
  sonic_rs::to_vec(value).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn deserialize_io<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, io::Error> {
  sonic_rs::from_slice(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn durable_write_options() -> WriteOptions {
  let mut opts = WriteOptions::default();
  opts.set_sync(true);
  opts.disable_wal(false);
  opts
}

fn write_batch_sync(db: &DB, batch: rocksdb::WriteBatch) -> Result<(), io::Error> {
  db.write_opt(batch, &durable_write_options())
    .map_err(|e| io::Error::other(e.to_string()))
}

fn decode_utf8(bytes: Vec<u8>, what: &str) -> Result<String, io::Error> {
  String::from_utf8(bytes)
    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{what}: {e}")))
}

fn snapshot_data_to_map(
  data: &[(Vec<u8>, Vec<u8>)],
) -> Result<BTreeMap<String, String>, io::Error> {
  let mut map = BTreeMap::new();
  for (key, value) in data {
    map.insert(
      decode_utf8(key.clone(), "snapshot key")?,
      decode_utf8(value.clone(), "snapshot value")?,
    );
  }
  Ok(map)
}

fn apply_memory_changes(map: &mut BTreeMap<String, String>, changes: Vec<DataChange>) {
  for change in changes {
    match change {
      DataChange::Set { key, value } => {
        map.insert(key, value);
      }
      DataChange::Delete { key } => {
        map.remove(&key);
      }
    }
  }
}

/// Snapshot file format: metadata + data stored together
#[derive(Serialize, Deserialize)]
struct SnapshotFile {
  meta: SnapshotMetaOf<TypeConfig>,
  data: Vec<(Vec<u8>, Vec<u8>)>,
}

fn snapshot_is_newer(candidate: &SnapshotFile, current: &SnapshotFile) -> bool {
  match (&candidate.meta.last_log_id, &current.meta.last_log_id) {
    (Some(candidate_log), Some(current_log)) => {
      candidate_log > current_log
        || (candidate_log == current_log && candidate.meta.snapshot_id > current.meta.snapshot_id)
    }
    (Some(_), None) => true,
    (None, Some(_)) => false,
    (None, None) => candidate.meta.snapshot_id > current.meta.snapshot_id,
  }
}

fn is_snapshot_candidate(path: &Path) -> bool {
  if !path.is_file() {
    return false;
  }

  path
    .file_name()
    .and_then(|name| name.to_str())
    .is_some_and(|name| !name.ends_with(SNAPSHOT_TMP_SUFFIX))
}

fn read_snapshot_file(path: &Path) -> Result<SnapshotFile, io::Error> {
  let file_bytes = fs::read(path)?;
  deserialize_io(&file_bytes)
}

fn read_latest_snapshot_file(snapshot_dir: &Path) -> Result<Option<SnapshotFile>, io::Error> {
  let mut latest: Option<SnapshotFile> = None;

  for entry in fs::read_dir(snapshot_dir)? {
    let entry = entry?;
    let path = entry.path();
    if !is_snapshot_candidate(&path) {
      continue;
    }

    match read_snapshot_file(&path) {
      Ok(snapshot_file) => {
        if latest
          .as_ref()
          .is_none_or(|current| snapshot_is_newer(&snapshot_file, current))
        {
          latest = Some(snapshot_file);
        }
      }
      Err(err) => {
        tracing::warn!(
          path = %path.display(),
          error = ?err,
          "skip unreadable persisted rocksdb snapshot"
        );
      }
    }
  }

  Ok(latest)
}

pub(crate) fn read_latest_snapshot_meta(
  snapshot_dir: &Path,
) -> Result<Option<SnapshotMetaOf<TypeConfig>>, io::Error> {
  Ok(read_latest_snapshot_file(snapshot_dir)?.map(|snapshot| snapshot.meta))
}

fn sync_dir(path: &Path) -> Result<(), io::Error> {
  match File::open(path) {
    Ok(dir) => dir.sync_all(),
    Err(err) if err.kind() == io::ErrorKind::PermissionDenied => Ok(()),
    Err(err) => Err(err),
  }
}

fn write_snapshot_file(
  snapshot_dir: &Path,
  snapshot_id: &str,
  snapshot_file: &SnapshotFile,
) -> Result<(), io::Error> {
  fs::create_dir_all(snapshot_dir)?;

  let snapshot_path = snapshot_dir.join(snapshot_id);
  let tmp_path = snapshot_dir.join(format!(
    "{snapshot_id}.{}{}",
    std::process::id(),
    SNAPSHOT_TMP_SUFFIX
  ));
  let file_bytes = serialize_io(snapshot_file)?;

  {
    let mut file = File::create(&tmp_path)?;
    file.write_all(&file_bytes)?;
    file.sync_all()?;
  }

  fs::rename(&tmp_path, &snapshot_path)?;
  sync_dir(snapshot_dir)?;

  Ok(())
}

fn read_last_applied_log(db: &DB) -> Result<Option<LogIdOf<TypeConfig>>, io::Error> {
  let cf = db
    .cf_handle(SM_META_CF)
    .ok_or_else(|| io::Error::other(format!("column family `{SM_META_CF}` not found")))?;

  db.get_cf(cf, LAST_APPLIED_LOG_KEY)
    .map_err(|e| io::Error::other(e.to_string()))?
    .map(|bytes| deserialize_io(&bytes))
    .transpose()
}

fn restore_snapshot_to_db(db: &DB, snapshot_file: &SnapshotFile) -> Result<(), io::Error> {
  let cf_data = db
    .cf_handle(SM_DATA_CF)
    .ok_or_else(|| io::Error::other(format!("column family `{SM_DATA_CF}` not found")))?;
  let cf_meta = db
    .cf_handle(SM_META_CF)
    .ok_or_else(|| io::Error::other(format!("column family `{SM_META_CF}` not found")))?;

  let last_applied_bytes = snapshot_file
    .meta
    .last_log_id
    .as_ref()
    .map(serialize_io)
    .transpose()?;
  let last_membership_bytes = serialize_io(&snapshot_file.meta.last_membership)?;

  let mut batch = rocksdb::WriteBatch::default();
  let iter = db.iterator_cf(cf_data, rocksdb::IteratorMode::Start);
  for item in iter {
    let (key, _) = item.map_err(|e| io::Error::other(e.to_string()))?;
    batch.delete_cf(cf_data, &key);
  }

  for (key, value) in &snapshot_file.data {
    batch.put_cf(cf_data, key, value);
  }

  if let Some(bytes) = last_applied_bytes {
    batch.put_cf(cf_meta, LAST_APPLIED_LOG_KEY, bytes);
  } else {
    batch.delete_cf(cf_meta, LAST_APPLIED_LOG_KEY);
  }
  batch.put_cf(cf_meta, LAST_MEMBERSHIP_KEY, last_membership_bytes);

  write_batch_sync(db, batch)
}

async fn recover_from_latest_snapshot_if_newer(
  db: Arc<DB>,
  snapshot_dir: &Path,
) -> Result<(), io::Error> {
  let Some(snapshot_file) = read_latest_snapshot_file(snapshot_dir)? else {
    return Ok(());
  };

  let current_last_applied = read_last_applied_log(&db)?;
  if snapshot_file.meta.last_log_id <= current_last_applied {
    return Ok(());
  }

  let snapshot_id = snapshot_file.meta.snapshot_id.clone();
  let snapshot_last_log_id = snapshot_file.meta.last_log_id.clone();

  TypeConfig::spawn_blocking(move || restore_snapshot_to_db(&db, &snapshot_file)).await??;

  tracing::info!(
    snapshot_id,
    ?snapshot_last_log_id,
    ?current_last_applied,
    "restored rocksdb state machine from persisted snapshot"
  );

  Ok(())
}

impl RaftSnapshotBuilder<TypeConfig> for RocksStateMachine {
  type SnapshotData = Cursor<Vec<u8>>;

  #[tracing::instrument(level = "trace", skip(self))]
  async fn build_snapshot(
    &mut self,
  ) -> Result<SnapshotOf<TypeConfig, Self::SnapshotData>, io::Error> {
    let (last_applied_log, last_membership) = self.get_meta()?;

    // UUID v7 is time-ordered, so the lexicographic snapshot_id tiebreaker in
    // `snapshot_is_newer` follows creation order.
    let snapshot_uuid = uuid::Uuid::now_v7();

    let snapshot_id = if let Some(ref last) = last_applied_log {
      format!(
        "{}-{}-{}",
        last.committed_leader_id(),
        last.index(),
        snapshot_uuid
      )
    } else {
      format!("--{}", snapshot_uuid)
    };

    let meta = SnapshotMetaOf::<TypeConfig> {
      last_log_id: last_applied_log,
      last_membership,
      snapshot_id: snapshot_id.clone(),
    };

    // Use RocksDB snapshot for consistent point-in-time view. Collecting,
    // serializing, and persisting the snapshot are all CPU/IO heavy, so run
    // the whole pipeline off the async runtime.
    let db = self.db.clone();
    let snapshot_dir = self.snapshot_dir.clone();
    let meta_for_file = meta.clone();
    let snapshot_id_for_file = snapshot_id;

    let data_bytes = TypeConfig::spawn_blocking(move || -> Result<Vec<u8>, io::Error> {
      let snapshot = db.snapshot();
      let cf_data = db
        .cf_handle(SM_DATA_CF)
        .expect("column family `sm_data` not found");

      let mut snapshot_data = Vec::new();
      let iter = snapshot.iterator_cf(cf_data, rocksdb::IteratorMode::Start);

      for item in iter {
        let (key, value) = item.map_err(|e| io::Error::other(e.to_string()))?;
        snapshot_data.push((key.to_vec(), value.to_vec()));
      }

      // Serialize both metadata and data together
      let snapshot_file = SnapshotFile {
        meta: meta_for_file,
        data: snapshot_data,
      };
      write_snapshot_file(&snapshot_dir, &snapshot_id_for_file, &snapshot_file)?;

      // Return snapshot with data-only for backward compatibility with the data field
      serialize_io(&snapshot_file.data)
    })
    .await??;

    Ok(SnapshotOf::<TypeConfig, Self::SnapshotData> {
      meta,
      snapshot: Cursor::new(data_bytes),
    })
  }
}

impl RaftStateMachine<TypeConfig> for RocksStateMachine {
  type SnapshotData = Cursor<Vec<u8>>;

  type SnapshotBuilder = Self;

  async fn applied_state(
    &mut self,
  ) -> Result<(Option<LogIdOf<TypeConfig>>, StoredMembershipOf<TypeConfig>), io::Error> {
    self.get_meta().map_err(|e| io::Error::other(e.to_string()))
  }

  async fn apply<Strm>(&mut self, mut entries: Strm) -> Result<(), io::Error>
  where
    Strm: Stream<Item = Result<EntryResponder<TypeConfig>, io::Error>> + Unpin + OptionalSend,
  {
    let mut data_changes = Vec::new();
    let mut last_applied_log = None;
    let mut last_membership: Option<StoredMembershipOf<TypeConfig>> = None;
    let mut responses = Vec::new();
    let db = self.db.clone();
    let cf_data = db
      .cf_handle(SM_DATA_CF)
      .ok_or_else(|| io::Error::other(format!("column family `{SM_DATA_CF}` not found")))?;
    let cf_meta = db
      .cf_handle(SM_META_CF)
      .ok_or_else(|| io::Error::other(format!("column family `{SM_META_CF}` not found")))?;
    let mut batch = rocksdb::WriteBatch::default();
    // Reads issued while applying task commands must observe writes from
    // earlier entries in this SAME batch: replicas chunk the entry stream
    // differently, so relying on the flushed DB alone would make replicas
    // diverge. The overlay holds this batch's pending writes.
    let mut overlay: HashMap<String, Option<String>> = HashMap::new();

    while let Some((entry, responder)) = entries.try_next().await? {
      tracing::debug!(%entry.log_id, "replicate to sm");

      last_applied_log = Some(entry.log_id());

      let response = match entry.payload {
        EntryPayload::Blank => types_kv::Response::none(),
        EntryPayload::Normal(req) => match req {
          types_kv::Request::Set { key, value } => {
            batch.put_cf(cf_data, key.as_bytes(), value.as_bytes());
            // Only clone the value when someone is waiting for the response;
            // followers apply entries without responders.
            let response = if responder.is_some() {
              types_kv::Response::new(value.clone())
            } else {
              types_kv::Response::none()
            };
            overlay.insert(key.clone(), Some(value.clone()));
            data_changes.push(DataChange::Set { key, value });
            response
          }
          types_kv::Request::Delete { key } => {
            batch.delete_cf(cf_data, key.as_bytes());
            overlay.insert(key.clone(), None);
            data_changes.push(DataChange::Delete { key });
            types_kv::Response::none()
          }
          task_cmd => {
            let mut read = |key: &str| -> Result<Option<String>, String> {
              if let Some(pending) = overlay.get(key) {
                return Ok(pending.clone());
              }
              db.get_cf(cf_data, key.as_bytes())
                .map_err(|err| err.to_string())?
                .map(|bytes| String::from_utf8(bytes).map_err(|err| err.to_string()))
                .transpose()
            };
            let (mutations, response) =
              tasks::apply_task_command(&mut read, task_cmd).map_err(io::Error::other)?;
            for mutation in mutations {
              match mutation.value {
                Some(value) => {
                  batch.put_cf(cf_data, mutation.key.as_bytes(), value.as_bytes());
                  overlay.insert(mutation.key.clone(), Some(value.clone()));
                  data_changes.push(DataChange::Set {
                    key: mutation.key,
                    value,
                  });
                }
                None => {
                  batch.delete_cf(cf_data, mutation.key.as_bytes());
                  overlay.insert(mutation.key.clone(), None);
                  data_changes.push(DataChange::Delete { key: mutation.key });
                }
              }
            }
            response
          }
        },
        EntryPayload::Membership(ref mem) => {
          last_membership = Some(StoredMembershipOf::<TypeConfig>::new(
            Some(entry.log_id),
            mem.clone(),
          ));
          types_kv::Response::none()
        }
      };

      if let Some(responder) = responder {
        responses.push((responder, response));
      }
    }

    if let Some(ref log_id) = last_applied_log {
      batch.put_cf(cf_meta, LAST_APPLIED_LOG_KEY, serialize_io(log_id)?);
    }

    if let Some(ref membership) = last_membership {
      batch.put_cf(cf_meta, LAST_MEMBERSHIP_KEY, serialize_io(membership)?);
    }

    TypeConfig::spawn_blocking(move || write_batch_sync(&db, batch)).await??;

    if !data_changes.is_empty() {
      let mut data = self.data.write().await;
      apply_memory_changes(&mut data, data_changes);
    }

    // Only send responses after successful write
    for (responder, response) in responses {
      responder.send(response);
    }

    Ok(())
  }

  async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
    self.clone()
  }

  async fn begin_receiving_snapshot(&mut self) -> Result<Self::SnapshotData, io::Error> {
    Ok(Cursor::new(Vec::new()))
  }

  async fn install_snapshot(
    &mut self,
    meta: &SnapshotMetaOf<TypeConfig>,
    snapshot: Self::SnapshotData,
  ) -> Result<(), io::Error> {
    tracing::info!(
      { snapshot_size = snapshot.get_ref().len() },
      "decoding snapshot for installation"
    );

    // Deserializing, rebuilding the in-memory map, restoring RocksDB, and
    // persisting the snapshot file are all CPU/IO heavy for large snapshots,
    // so run the whole pipeline off the async runtime.
    let db = self.db.clone();
    let snapshot_dir = self.snapshot_dir.clone();
    let meta_for_file = meta.clone();

    let data_map =
      TypeConfig::spawn_blocking(move || -> Result<BTreeMap<String, String>, io::Error> {
        let snapshot_data: Vec<(Vec<u8>, Vec<u8>)> = deserialize_io(snapshot.get_ref())?;
        let data_map = snapshot_data_to_map(&snapshot_data)?;
        let snapshot_file = SnapshotFile {
          meta: meta_for_file,
          data: snapshot_data,
        };

        restore_snapshot_to_db(&db, &snapshot_file)?;
        write_snapshot_file(
          &snapshot_dir,
          &snapshot_file.meta.snapshot_id,
          &snapshot_file,
        )?;
        Ok(data_map)
      })
      .await??;

    {
      let mut data = self.data.write().await;
      *data = data_map;
    }

    Ok(())
  }

  async fn get_current_snapshot(
    &mut self,
  ) -> Result<Option<SnapshotOf<TypeConfig, Self::SnapshotData>>, io::Error> {
    // Reading and re-serializing persisted snapshots is blocking file IO plus
    // CPU-heavy JSON work; keep it off the async runtime.
    let snapshot_dir = self.snapshot_dir.clone();
    let snapshot = TypeConfig::spawn_blocking(
      move || -> Result<Option<(SnapshotMetaOf<TypeConfig>, Vec<u8>)>, io::Error> {
        let Some(snapshot_file) = read_latest_snapshot_file(&snapshot_dir)? else {
          return Ok(None);
        };

        // Serialize data for snapshot field
        let data_bytes = serialize_io(&snapshot_file.data)?;
        Ok(Some((snapshot_file.meta, data_bytes)))
      },
    )
    .await??;

    Ok(snapshot.map(
      |(meta, data_bytes)| SnapshotOf::<TypeConfig, Self::SnapshotData> {
        meta,
        snapshot: Cursor::new(data_bytes),
      },
    ))
  }
}

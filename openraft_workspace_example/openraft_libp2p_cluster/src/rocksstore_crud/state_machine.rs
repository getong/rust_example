//! RocksDB-backed state machine implementation.

use std::{
  collections::{BTreeMap, HashMap},
  fmt::Debug,
  fs,
  fs::File,
  io,
  io::{Cursor, Write},
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
};

use futures::{Stream, TryStreamExt};
use openraft::{
  EntryPayload, OptionalSend, RaftSnapshotBuilder, StorageError,
  alias::{LogIdOf, SnapshotMetaOf, SnapshotOf, StoredMembershipOf},
  entry::RaftEntry,
  storage::{EntryResponder, RaftStateMachine},
  type_config::TypeConfigExt,
};
use rayon::prelude::*;
use rocksdb::{DB, WriteOptions};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::{OperationTimer, TypeConfig};
use crate::{tasks, types_kv};

const SM_META_CF: &str = "sm_meta";
const SM_DATA_CF: &str = "sm_data";
const LAST_APPLIED_LOG_KEY: &str = "last_applied_log";
const LAST_MEMBERSHIP_KEY: &str = "last_membership";
const SNAPSHOT_TMP_SUFFIX: &str = ".tmp";
/// Snapshot files carry a monotonically increasing epoch prefix
/// (`epoch_0000000042-<snapshot_id>`), restored from the directory scan at
/// startup. The epoch makes creation order explicit in the file name — no
/// need to open and deserialize each file to order them — and gives
/// concurrent builders distinct target names.
const SNAPSHOT_EPOCH_PREFIX: &str = "epoch_";
/// Snapshot files kept after a successful write; older ones are pruned so
/// the snapshot directory does not grow without bound.
const SNAPSHOT_RETAIN_COUNT: usize = 3;
const SNAPSHOT_ZSTD_LEVEL: i32 = 3;
const ZSTD_FRAME_MAGIC: &[u8; 4] = b"\x28\xb5\x2f\xfd";

/// Raw key/value pair as yielded by a RocksDB iterator.
type RawKv = (Box<[u8]>, Box<[u8]>);

/// State machine backed by RocksDB for full persistence.
/// All application data is stored directly in the `sm_data` column family.
/// Snapshots are persisted to the `snapshot_dir` directory.
#[derive(Debug, Clone)]
pub struct RocksStateMachine {
  db: Arc<DB>,
  snapshot_dir: PathBuf,
  data: Arc<RwLock<BTreeMap<String, String>>>,
  /// Next snapshot epoch; see `SNAPSHOT_EPOCH_PREFIX`.
  snapshot_epoch: Arc<AtomicU64>,
}

#[derive(Debug)]
enum DataChange {
  Set { key: String, value: String },
  Delete { key: String },
}

impl RocksStateMachine {
  /// Handle to the primary RocksDB. Used by the `KvData` secondary reader to
  /// compare sequence numbers and skip needless catch-up syncs.
  pub fn db(&self) -> Arc<DB> {
    self.db.clone()
  }

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
    let snapshot_epoch = next_snapshot_epoch(&snapshot_dir)?;
    recover_from_latest_snapshot_if_newer(db.clone(), &snapshot_dir).await?;

    let data = TypeConfig::spawn_blocking({
      let db = db.clone();
      move || -> Result<BTreeMap<String, String>, io::Error> {
        let cf_data = db
          .cf_handle(SM_DATA_CF)
          .ok_or_else(|| io::Error::other(format!("column family `{SM_DATA_CF}` not found")))?;
        let iter = db.iterator_cf(cf_data, rocksdb::IteratorMode::Start);
        // RocksDB iteration is inherently sequential; collect the raw bytes
        // first, then decode them on the rayon pool.
        let raw: Vec<RawKv> = iter
          .collect::<Result<_, _>>()
          .map_err(|e| io::Error::other(e.to_string()))?;
        raw
          .into_par_iter()
          .map(|(key, value)| {
            Ok((
              decode_utf8(key.into_vec(), "snapshot key")?,
              decode_utf8(value.into_vec(), "snapshot value")?,
            ))
          })
          .collect()
      }
    })
    .await??;

    Ok(Self {
      db,
      snapshot_dir,
      data: Arc::new(RwLock::new(data)),
      snapshot_epoch: Arc::new(AtomicU64::new(snapshot_epoch)),
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

  #[test]
  fn persisted_snapshot_is_zstd_compressed_and_round_trips() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let snapshot = SnapshotFile {
      meta: SnapshotMetaOf::<TypeConfig> {
        last_log_id: Some(log_id(7)),
        last_membership: StoredMembershipOf::<TypeConfig>::default(),
        snapshot_id: "compressed-snapshot".to_string(),
      },
      data: vec![(b"key".to_vec(), vec![b'x'; 32 * 1024])],
    };
    let raw_size = serialize_io(&snapshot)
      .expect("serialize raw snapshot")
      .len();

    let persisted_size =
      write_snapshot_file(temp.path(), "snapshot", &snapshot).expect("write snapshot");
    let bytes = fs::read(temp.path().join("snapshot")).expect("read snapshot bytes");
    assert!(bytes.starts_with(ZSTD_FRAME_MAGIC));
    assert_eq!(persisted_size, bytes.len() as u64);
    assert!(bytes.len() < raw_size);

    let restored = read_snapshot_file(&temp.path().join("snapshot")).expect("read snapshot");
    assert_eq!(restored.meta.last_log_id, snapshot.meta.last_log_id);
    assert_eq!(restored.meta.snapshot_id, snapshot.meta.snapshot_id);
    assert_eq!(restored.data, snapshot.data);
  }

  #[test]
  fn legacy_uncompressed_snapshot_still_loads() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let path = temp.path().join("legacy-snapshot");
    let snapshot = SnapshotFile {
      meta: SnapshotMetaOf::<TypeConfig> {
        last_log_id: Some(log_id(3)),
        last_membership: StoredMembershipOf::<TypeConfig>::default(),
        snapshot_id: "legacy-snapshot".to_string(),
      },
      data: vec![(b"alpha".to_vec(), b"one".to_vec())],
    };
    fs::write(
      &path,
      serialize_io(&snapshot).expect("serialize legacy snapshot"),
    )
    .expect("write legacy snapshot");

    let restored = read_snapshot_file(&path).expect("read legacy snapshot");
    assert_eq!(restored.meta.last_log_id, snapshot.meta.last_log_id);
    assert_eq!(restored.meta.snapshot_id, snapshot.meta.snapshot_id);
    assert_eq!(restored.data, snapshot.data);
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

/// Write options for raft storage. The WAL is always enabled; whether each
/// write batch also fsyncs it is governed by the hot-reloadable
/// `rocksdb_sync_writes` runtime config (default `true` = strong single-node
/// durability; `false` trades crash durability for throughput and relies on
/// raft replication — see [`crate::runtime_config::RuntimeConfig`]).
fn durable_write_options() -> WriteOptions {
  let mut opts = WriteOptions::default();
  opts.set_sync(crate::runtime_config::current().rocksdb_sync_writes);
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
  data
    .par_iter()
    .map(|(key, value)| {
      Ok((
        decode_utf8(key.clone(), "snapshot key")?,
        decode_utf8(value.clone(), "snapshot value")?,
      ))
    })
    .collect()
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
  if file_bytes.starts_with(ZSTD_FRAME_MAGIC) {
    let decoded = zstd::stream::decode_all(file_bytes.as_slice())?;
    deserialize_io(&decoded)
  } else {
    // Snapshots written before local compression was enabled are raw JSON.
    deserialize_io(&file_bytes)
  }
}

fn read_latest_snapshot_file(snapshot_dir: &Path) -> Result<Option<SnapshotFile>, io::Error> {
  let mut candidates = Vec::new();
  for entry in fs::read_dir(snapshot_dir)? {
    let path = entry?.path();
    if is_snapshot_candidate(&path) {
      candidates.push(path);
    }
  }

  // Reading + JSON-decoding each multi-megabyte snapshot file dominates
  // here; parse the candidates on the rayon pool and keep the newest.
  // `snapshot_is_newer` is a total order, so the reduction result does not
  // depend on rayon's split order.
  Ok(
    candidates
      .into_par_iter()
      .filter_map(|path| match read_snapshot_file(&path) {
        Ok(snapshot_file) => Some(snapshot_file),
        Err(err) => {
          tracing::warn!(
            path = %path.display(),
            error = ?err,
            "skip unreadable persisted rocksdb snapshot"
          );
          None
        }
      })
      .reduce_with(|current, candidate| {
        if snapshot_is_newer(&candidate, &current) {
          candidate
        } else {
          current
        }
      }),
  )
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

/// File name for a snapshot: monotonically increasing epoch prefix plus the
/// snapshot id, so creation order is visible in the name and two concurrent
/// builds can never target the same path.
fn snapshot_file_name(epoch: u64, snapshot_id: &str) -> String {
  format!("{SNAPSHOT_EPOCH_PREFIX}{epoch:010}-{snapshot_id}")
}

fn parse_snapshot_epoch(name: &str) -> Option<u64> {
  name
    .strip_prefix(SNAPSHOT_EPOCH_PREFIX)?
    .split('-')
    .next()?
    .parse()
    .ok()
}

/// First epoch this process should use: one past the highest epoch found in
/// the snapshot directory. Legacy files without an epoch prefix count as 0.
fn next_snapshot_epoch(snapshot_dir: &Path) -> Result<u64, io::Error> {
  let mut max_epoch = 0u64;
  for entry in fs::read_dir(snapshot_dir)? {
    let entry = entry?;
    if let Some(epoch) = entry.file_name().to_str().and_then(parse_snapshot_epoch) {
      max_epoch = max_epoch.max(epoch);
    }
  }
  Ok(max_epoch + 1)
}

/// Delete all but the newest `keep` snapshot files, plus stale tmp files
/// left behind by crashed writers. Newness is (epoch, file name): epochs are
/// monotonic across process restarts and the snapshot id embeds a
/// time-ordered UUID v7, so lexicographic name order is creation order
/// within an epoch. Legacy un-prefixed files sort as epoch 0 and age out
/// first.
fn prune_old_snapshots(snapshot_dir: &Path, keep: usize) -> Result<(), io::Error> {
  let own_tmp_marker = format!(".{}{}", std::process::id(), SNAPSHOT_TMP_SUFFIX);
  let mut candidates: Vec<(u64, String, PathBuf)> = Vec::new();
  for entry in fs::read_dir(snapshot_dir)? {
    let entry = entry?;
    let path = entry.path();
    let Some(name) = path
      .file_name()
      .and_then(|n| n.to_str())
      .map(str::to_string)
    else {
      continue;
    };
    if name.ends_with(SNAPSHOT_TMP_SUFFIX) {
      // Tmp names embed the writer's pid; one from another pid is a
      // leftover from a crash mid-write.
      if !name.ends_with(&own_tmp_marker) {
        let _ = fs::remove_file(&path);
      }
      continue;
    }
    if !path.is_file() {
      continue;
    }
    candidates.push((parse_snapshot_epoch(&name).unwrap_or(0), name, path));
  }

  if candidates.len() <= keep {
    return Ok(());
  }
  candidates.sort_by(|a, b| (b.0, &b.1).cmp(&(a.0, &a.1)));
  for (_, name, path) in candidates.into_iter().skip(keep) {
    match fs::remove_file(&path) {
      Ok(()) => tracing::debug!(snapshot = %name, "pruned old rocksdb snapshot file"),
      Err(err) => {
        tracing::warn!(snapshot = %name, error = ?err, "failed to prune old rocksdb snapshot file")
      }
    }
  }
  sync_dir(snapshot_dir)
}

fn write_snapshot_file(
  snapshot_dir: &Path,
  snapshot_file_name: &str,
  snapshot_file: &SnapshotFile,
) -> Result<u64, io::Error> {
  fs::create_dir_all(snapshot_dir)?;

  let snapshot_path = snapshot_dir.join(snapshot_file_name);
  let tmp_path = snapshot_dir.join(format!(
    "{snapshot_file_name}.{}{}",
    std::process::id(),
    SNAPSHOT_TMP_SUFFIX
  ));
  let file_bytes = serialize_io(snapshot_file)?;
  let file = File::create(&tmp_path)?;
  let mut encoder = zstd::stream::write::Encoder::new(file, SNAPSHOT_ZSTD_LEVEL)?;
  encoder.write_all(&file_bytes)?;
  let file = encoder.finish()?;
  file.sync_all()?;
  let persisted_size = file.metadata()?.len();

  fs::rename(&tmp_path, &snapshot_path)?;
  sync_dir(snapshot_dir)?;

  Ok(persisted_size)
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
    let _timer = OperationTimer::start("rocksdb_snapshot_build_duration_seconds");
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
    let epoch = self.snapshot_epoch.fetch_add(1, Ordering::SeqCst);
    let file_name = snapshot_file_name(epoch, &snapshot_id);

    let (data_bytes, persisted_size) =
      TypeConfig::spawn_blocking(move || -> Result<(Vec<u8>, u64), io::Error> {
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
        let persisted_size = write_snapshot_file(&snapshot_dir, &file_name, &snapshot_file)?;
        prune_old_snapshots(&snapshot_dir, SNAPSHOT_RETAIN_COUNT)?;

        // Return snapshot with data-only for backward compatibility with the data field
        Ok((serialize_io(&snapshot_file.data)?, persisted_size))
      })
      .await??;

    metrics::histogram!("rocksdb_snapshot_size_bytes", "kind" => "wire")
      .record(data_bytes.len() as f64);
    metrics::histogram!("rocksdb_snapshot_size_bytes", "kind" => "persisted")
      .record(persisted_size as f64);

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
    let _timer = OperationTimer::start("rocksdb_sm_apply_duration_seconds");
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
    let mut schedule_event = false;

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
          types_kv::Request::Task(task_cmd) => {
            schedule_event |= tasks::is_schedule_event(&task_cmd);
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

    // The apply feed is the authoritative change stream for schedulable
    // work: wake the local scheduler after the batch is durable. On
    // followers (no scheduler subscribed) this is a no-op.
    if schedule_event {
      tasks::events::notify_schedule();
    }

    Ok(())
  }

  async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
    self.clone()
  }

  async fn install_snapshot(
    &mut self,
    meta: &SnapshotMetaOf<TypeConfig>,
    snapshot: Self::SnapshotData,
  ) -> Result<(), io::Error> {
    let _timer = OperationTimer::start("rocksdb_snapshot_install_duration_seconds");
    metrics::histogram!("rocksdb_snapshot_size_bytes", "kind" => "wire")
      .record(snapshot.get_ref().len() as f64);
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
    let epoch = self.snapshot_epoch.fetch_add(1, Ordering::SeqCst);

    let data_map =
      TypeConfig::spawn_blocking(move || -> Result<BTreeMap<String, String>, io::Error> {
        let snapshot_data: Vec<(Vec<u8>, Vec<u8>)> = deserialize_io(snapshot.get_ref())?;
        let data_map = snapshot_data_to_map(&snapshot_data)?;
        let snapshot_file = SnapshotFile {
          meta: meta_for_file,
          data: snapshot_data,
        };

        restore_snapshot_to_db(&db, &snapshot_file)?;
        let file_name = snapshot_file_name(epoch, &snapshot_file.meta.snapshot_id);
        let persisted_size = write_snapshot_file(&snapshot_dir, &file_name, &snapshot_file)?;
        metrics::histogram!("rocksdb_snapshot_size_bytes", "kind" => "persisted")
          .record(persisted_size as f64);
        prune_old_snapshots(&snapshot_dir, SNAPSHOT_RETAIN_COUNT)?;
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

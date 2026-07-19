//! RocksDB-backed Raft logs and state machine.

use std::{
  ffi::OsString,
  fs,
  path::{Path, PathBuf},
  sync::Arc,
  time::Instant,
};

use anyhow::Context;
use openraft::{
  RaftLogReader, ReadPolicy,
  alias::LogIdOf,
  entry::RaftEntry,
  storage::{RaftLogStorage, RaftStateMachine},
  type_config::TypeConfigExt,
};
use rayon::prelude::*;
use rocksdb::{ColumnFamilyRef, DB};

use crate::{
  rocksstore_crud::{
    RocksStateMachine, TypeConfig,
    log_store::RocksLogStore,
    options::{secondary_cf_descriptors, secondary_db_options},
    state_machine::read_latest_snapshot_meta,
  },
  typ::{LinearizableReadError, Raft, RaftError, StoredMembership},
};

pub type LogStore = RocksLogStore<TypeConfig>;
pub type StateMachineStore = RocksStateMachine;

const SM_DATA_CF: &str = "sm_data";
/// Rebuild (reopen) the secondary reader when it is still this many sequence
/// numbers behind the primary after a catch-up attempt. Secondary mode
/// catches up by replaying the primary's WAL; when the primary has purged
/// old WAL files (post-checkpoint/snapshot) the secondary can no longer
/// close the gap incrementally and must be reopened against the current SST
/// set.
const SECONDARY_REBUILD_GAP: u64 = 8192;

/// Raw key/value pair as yielded by a RocksDB iterator.
type RawKv = (Box<[u8]>, Box<[u8]>);

#[derive(Debug, Clone)]
pub struct KvData {
  /// Swappable secondary handle: reads `load()` it per operation, and a
  /// rebuild replaces it atomically while in-flight readers finish on the
  /// old instance.
  db: Arc<arc_swap::ArcSwap<DB>>,
  /// Same-process primary the secondary reader mirrors. Sequence numbers
  /// from it decide whether a catch-up sync is needed at all.
  primary: Arc<DB>,
  primary_path: Arc<PathBuf>,
  /// Single-flight guard for catch-up: under a concurrent read burst only
  /// one reader performs the manifest sync; the rest wait on the lock and
  /// then see the fresh sequence number, amortizing one catch-up across the
  /// whole batch instead of syncing once per reader.
  catch_up_lock: Arc<std::sync::Mutex<()>>,
  /// Counts secondary rebuilds; each rebuild opens a fresh
  /// `<base>.secondary.<epoch>` directory because the old instance may still
  /// serve in-flight readers.
  rebuild_epoch: Arc<std::sync::atomic::AtomicU64>,
  /// When the previous rebuild happened, for the
  /// `kv_secondary_rebuild_interval_seconds` histogram: a shrinking interval
  /// means the write volume outruns `SECONDARY_REBUILD_GAP` and the constant
  /// (or the write batching) needs a second look.
  last_rebuild_at: Arc<std::sync::Mutex<Option<Instant>>>,
}

impl KvData {
  pub fn open<P: AsRef<Path>>(primary: Arc<DB>, primary_path: P) -> anyhow::Result<Self> {
    let primary_path = primary_path.as_ref();
    let secondary_path = secondary_db_dir(primary_path);
    remove_stale_secondary_dirs(&secondary_path);

    let db = open_secondary(primary_path, &secondary_path)?;

    let kv_data = Self {
      db: Arc::new(arc_swap::ArcSwap::from_pointee(db)),
      primary,
      primary_path: Arc::new(primary_path.to_path_buf()),
      catch_up_lock: Arc::new(std::sync::Mutex::new(())),
      rebuild_epoch: Arc::new(std::sync::atomic::AtomicU64::new(0)),
      last_rebuild_at: Arc::new(std::sync::Mutex::new(None)),
    };
    catch_up(&kv_data.db.load())?;
    Ok(kv_data)
  }

  pub async fn get(&self, key: &str) -> anyhow::Result<Option<String>> {
    let this = self.clone();
    let key = key.to_string();
    let started = Instant::now();
    let result = TypeConfig::spawn_blocking(move || {
      let db = this.synced_db()?;
      let cf = sm_data_cf(&db)?;
      db.get_cf(&cf, key.as_bytes())
        .context("read rocksdb kv value")?
        .map(|value| decode_utf8(value.as_ref(), "value"))
        .transpose()
    })
    .await
    .context("join rocksdb kv get task")?;
    record_kv_read_duration("get", started);
    result
  }

  pub async fn contains_key(&self, key: &str) -> anyhow::Result<bool> {
    self.get(key).await.map(|value| value.is_some())
  }

  pub async fn entries(&self) -> anyhow::Result<Vec<(String, String)>> {
    let this = self.clone();
    let started = Instant::now();
    let result = TypeConfig::spawn_blocking(move || {
      let db = this.synced_db()?;
      let cf = sm_data_cf(&db)?;
      let iter = db.iterator_cf(&cf, rocksdb::IteratorMode::Start);
      let raw: Vec<RawKv> = iter
        .collect::<Result<_, _>>()
        .context("iterate rocksdb kv data")?;
      decode_kv_entries(raw)
    })
    .await
    .context("join rocksdb kv entries task")?;
    record_kv_read_duration("entries", started);
    result
  }

  pub async fn entries_with_prefix(&self, prefix: String) -> anyhow::Result<Vec<(String, String)>> {
    let this = self.clone();
    let started = Instant::now();
    let result = TypeConfig::spawn_blocking(move || {
      let db = this.synced_db()?;
      let cf = sm_data_cf(&db)?;
      let iter = db.iterator_cf(
        &cf,
        rocksdb::IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward),
      );
      let mut raw = Vec::new();
      for item in iter {
        let (key, value) = item.context("iterate rocksdb kv data")?;
        if !key.starts_with(prefix.as_bytes()) {
          break;
        }
        raw.push((key, value));
      }
      decode_kv_entries(raw)
    })
    .await
    .context("join rocksdb kv entries_with_prefix task")?;
    record_kv_read_duration("entries_with_prefix", started);
    result
  }

  /// Blocking. Return the secondary handle to read from, caught up with the
  /// same-process primary. Comparing `latest_sequence_number` is a lock-free
  /// in-memory read, so idle/read-mostly workloads skip the per-read
  /// manifest sync entirely — while a read issued right after a committed
  /// write always catches up first, preserving read-your-writes (which a
  /// time-based throttle would break).
  ///
  /// Single-flight: concurrent lagging readers serialize on the lock and
  /// re-check the sequence numbers after acquiring it, so a burst of N reads
  /// behind one write performs ONE catch-up, not N.
  ///
  /// When the secondary cannot catch up (the primary purged the WAL files
  /// secondary mode replays) or stays more than `SECONDARY_REBUILD_GAP`
  /// sequence numbers behind, it is rebuilt: reopened against the primary's
  /// current SST set and swapped in atomically.
  fn synced_db(&self) -> anyhow::Result<Arc<DB>> {
    let db = self.db.load_full();
    if db.latest_sequence_number() >= self.primary.latest_sequence_number() {
      return Ok(db);
    }

    let _guard = self
      .catch_up_lock
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    // Double-checked: whoever held the lock first has already caught up (or
    // rebuilt the secondary — reload the handle).
    let db = self.db.load_full();
    if db.latest_sequence_number() >= self.primary.latest_sequence_number() {
      return Ok(db);
    }

    match catch_up(&db) {
      Ok(()) => {
        let gap = self
          .primary
          .latest_sequence_number()
          .saturating_sub(db.latest_sequence_number());
        if gap <= SECONDARY_REBUILD_GAP {
          return Ok(db);
        }
        tracing::warn!(
          gap,
          primary = %self.primary_path.display(),
          "rocksdb secondary reader still far behind after catch-up; rebuilding"
        );
      }
      Err(err) => {
        tracing::warn!(
          error = ?err,
          primary = %self.primary_path.display(),
          "rocksdb secondary reader catch-up failed; rebuilding"
        );
      }
    }
    match self.rebuild_secondary() {
      Ok(db) => Ok(db),
      // Graceful degradation: a failed rebuild (disk full, primary mid-
      // compaction, ...) must not make reads fail while the data is right
      // there in the same process. Serve this read from the primary — giving
      // up read/write isolation for it, but not availability — and let the
      // next lagging read retry the rebuild.
      Err(err) => {
        tracing::error!(
          error = ?err,
          primary = %self.primary_path.display(),
          "rocksdb secondary rebuild failed; falling back to reading the primary"
        );
        Ok(self.primary_fallback())
      }
    }
  }

  /// Degraded-mode read handle: the same-process primary. Reads on it are
  /// thread-safe but contend with writes for the block cache, so this is
  /// only used while the secondary cannot be rebuilt.
  fn primary_fallback(&self) -> Arc<DB> {
    metrics::counter!("kv_primary_fallback_read_total").increment(1);
    self.primary.clone()
  }

  /// Blocking; caller holds `catch_up_lock`. Open a fresh secondary
  /// instance in a new epoch directory, catch it up, and swap it in. The old
  /// instance stays alive until its last in-flight reader drops it; its
  /// directory is removed on the next `open` (process restart).
  fn rebuild_secondary(&self) -> anyhow::Result<Arc<DB>> {
    let epoch = self
      .rebuild_epoch
      .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
      + 1;
    let mut name = secondary_db_dir(&self.primary_path)
      .file_name()
      .map(OsString::from)
      .unwrap_or_else(|| OsString::from("rocksdb.secondary"));
    name.push(format!(".{epoch}"));
    let secondary_path = self.primary_path.with_file_name(name);

    let db = open_secondary(&self.primary_path, &secondary_path)?;
    catch_up(&db)?;
    let db = Arc::new(db);
    self.db.store(db.clone());
    metrics::counter!("kv_secondary_rebuild_total").increment(1);
    // Time between consecutive rebuilds: the signal for whether
    // `SECONDARY_REBUILD_GAP` fits the write volume (frequent rebuilds mean
    // the secondary keeps falling irrecoverably behind).
    let now = Instant::now();
    if let Some(previous) = self
      .last_rebuild_at
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .replace(now)
    {
      metrics::histogram!("kv_secondary_rebuild_interval_seconds")
        .record(now.duration_since(previous).as_secs_f64());
    }
    tracing::info!(
      secondary = %secondary_path.display(),
      sequence = db.latest_sequence_number(),
      "rebuilt rocksdb secondary reader"
    );
    Ok(db)
  }
}

fn open_secondary(primary_path: &Path, secondary_path: &Path) -> anyhow::Result<DB> {
  if let Some(parent) = secondary_path.parent() {
    fs::create_dir_all(parent)
      .with_context(|| format!("create rocksdb secondary parent: {}", parent.display()))?;
  }
  fs::create_dir_all(secondary_path)
    .with_context(|| format!("create rocksdb secondary dir: {}", secondary_path.display()))?;

  let opts = secondary_db_options();
  DB::open_cf_descriptors_as_secondary(
    &opts,
    primary_path,
    secondary_path,
    secondary_cf_descriptors(),
  )
  .with_context(|| {
    format!(
      "open rocksdb secondary reader: primary={}, secondary={}",
      primary_path.display(),
      secondary_path.display()
    )
  })
}

/// Remove leftover secondary directories (the base dir and any
/// `<base>.<epoch>` rebuild dirs) from a previous process run. Best-effort:
/// a failure only wastes disk, so it is logged, not propagated.
fn remove_stale_secondary_dirs(secondary_base: &Path) {
  let Some(base_name) = secondary_base.file_name().and_then(|n| n.to_str()) else {
    return;
  };
  let Some(parent) = secondary_base.parent() else {
    return;
  };
  let Ok(entries) = fs::read_dir(parent) else {
    return;
  };
  for entry in entries.flatten() {
    let name = entry.file_name();
    let Some(name) = name.to_str() else { continue };
    if name == base_name || name.starts_with(&format!("{base_name}.")) {
      if let Err(err) = fs::remove_dir_all(entry.path()) {
        tracing::warn!(
          path = %entry.path().display(),
          error = ?err,
          "failed to remove stale rocksdb secondary dir"
        );
      }
    }
  }
}

/// KV read latency through the secondary reader, including the (throttled)
/// catch-up. `op` is a static label: get / entries / entries_with_prefix.
fn record_kv_read_duration(op: &'static str, started: Instant) {
  metrics::histogram!("kv_read_duration_seconds", "op" => op)
    .record(started.elapsed().as_secs_f64());
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

/// Below this many pairs a scan decodes inline; whole-table scans past it
/// amortize the rayon pool dispatch. RocksDB iteration itself stays
/// sequential — only the UTF-8 decode fans out.
const PAR_DECODE_MIN_LEN: usize = 1024;

fn decode_kv_entries(raw: Vec<RawKv>) -> anyhow::Result<Vec<(String, String)>> {
  let decode = |(key, value): RawKv| Ok((decode_utf8(&key, "key")?, decode_utf8(&value, "value")?));
  if raw.len() < PAR_DECODE_MIN_LEN {
    raw.into_iter().map(decode).collect()
  } else {
    raw.into_par_iter().map(decode).collect()
  }
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

  let opts = secondary_db_options();
  let db = DB::open_cf_descriptors_read_only(&opts, &db_path, secondary_cf_descriptors(), false)
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
  let kv_data = KvData::open(state_machine.db(), &db_path)?;
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

/// Retry budget for the ReadIndex quorum probe. openraft confirms
/// leadership with one-shot heartbeat RPCs whose timeout is a single
/// `heartbeat_interval`, so on a loaded host a probe round can miss quorum
/// even while writes commit fine. `QuorumNotEnough` is that transient
/// probe-round failure, not a partition verdict — retry it briefly before
/// surfacing an error to the caller.
const LINEARIZABLE_READ_ATTEMPTS: u32 = 5;
const LINEARIZABLE_READ_BACKOFF: std::time::Duration = std::time::Duration::from_millis(200);

pub async fn ensure_linearizable_read(raft: &Raft) -> Result<(), RaftError<LinearizableReadError>> {
  let mut backoff = LINEARIZABLE_READ_BACKOFF;
  for attempt in 1 .. {
    let err = match raft.get_read_linearizer(ReadPolicy::ReadIndex).await {
      Ok(linearizer) => {
        return linearizer
          .await_ready(raft)
          .await
          .map(|_| ())
          .map_err(RaftError::Fatal);
      }
      Err(err) => err,
    };
    // ForwardToLeader must reach the caller immediately (it carries the
    // leader hint) and Fatal is final; only the probe-round miss retries.
    let transient = matches!(
      &err,
      RaftError::APIError(LinearizableReadError::QuorumNotEnough(_))
    );
    if !transient || attempt >= LINEARIZABLE_READ_ATTEMPTS {
      return Err(err);
    }
    tracing::debug!(
      attempt,
      backoff_ms = backoff.as_millis() as u64,
      "linearizable read probe missed quorum; retrying"
    );
    tokio::time::sleep(backoff).await;
    backoff = backoff.saturating_mul(2);
  }
  unreachable!("loop returns on success, non-transient error, or attempt cap")
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

  const STORE_CFS: [&str; 4] = ["meta", "sm_meta", SM_DATA_CF, "logs"];

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
    let db = Arc::new(DB::open_cf_descriptors(&opts, &primary_path, cfs).expect("open primary"));
    let cf = db.cf_handle(SM_DATA_CF).expect("sm_data cf");
    db.put_cf(&cf, b"alpha", b"one").expect("write alpha");

    let kv_data = KvData::open(db.clone(), &primary_path).expect("open kv data");
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
  async fn kv_data_reads_are_read_your_writes() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let primary_path = temp.path().join("primary");

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let cfs = STORE_CFS
      .into_iter()
      .map(|name| ColumnFamilyDescriptor::new(name, Options::default()));
    let db = Arc::new(DB::open_cf_descriptors(&opts, &primary_path, cfs).expect("open primary"));
    let cf = db.cf_handle(SM_DATA_CF).expect("sm_data cf");
    db.put_cf(&cf, b"alpha", b"one").expect("write alpha");

    let kv_data = KvData::open(db.clone(), &primary_path).expect("open kv data");

    // Every committed primary write must be visible to the very next read —
    // no throttle window, no sleep.
    for value in ["two", "three", "four"] {
      db.put_cf(&cf, b"alpha", value.as_bytes()).expect("write");
      assert_eq!(
        kv_data.get("alpha").await.expect("get alpha"),
        Some(value.to_string()),
        "read right after write must observe it"
      );
    }

    // After a read the secondary is at the primary's sequence number, so a
    // write-free read skips the catch-up sync entirely and still succeeds.
    assert_eq!(
      kv_data.db.load().latest_sequence_number(),
      db.latest_sequence_number()
    );
    assert_eq!(
      kv_data.get("alpha").await.expect("get alpha again"),
      Some("four".to_string())
    );
  }

  #[tokio::test]
  async fn kv_data_rebuilds_secondary_and_keeps_serving() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let primary_path = temp.path().join("primary");

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let cfs = STORE_CFS
      .into_iter()
      .map(|name| ColumnFamilyDescriptor::new(name, Options::default()));
    let db = Arc::new(DB::open_cf_descriptors(&opts, &primary_path, cfs).expect("open primary"));
    let cf = db.cf_handle(SM_DATA_CF).expect("sm_data cf");
    db.put_cf(&cf, b"alpha", b"one").expect("write alpha");

    let kv_data = KvData::open(db.clone(), &primary_path).expect("open kv data");
    let old_db = kv_data.db.load_full();

    // Force a rebuild (the path taken when catch-up fails or the gap stays
    // too large): the swapped-in secondary must serve reads, including data
    // written after the swap.
    kv_data.rebuild_secondary().expect("rebuild secondary");
    assert!(!Arc::ptr_eq(&old_db, &kv_data.db.load_full()));

    db.put_cf(&cf, b"beta", b"two").expect("write beta");
    assert_eq!(
      kv_data.get("alpha").await.expect("get alpha"),
      Some("one".to_string())
    );
    assert_eq!(
      kv_data.get("beta").await.expect("get beta"),
      Some("two".to_string())
    );
  }

  #[tokio::test]
  async fn kv_data_falls_back_to_primary_when_rebuild_fails() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let primary_path = temp.path().join("primary");

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let cfs = STORE_CFS
      .into_iter()
      .map(|name| ColumnFamilyDescriptor::new(name, Options::default()));
    let db = Arc::new(DB::open_cf_descriptors(&opts, &primary_path, cfs).expect("open primary"));
    let cf = db.cf_handle(SM_DATA_CF).expect("sm_data cf");
    db.put_cf(&cf, b"alpha", b"one").expect("write alpha");

    let kv_data = KvData::open(db.clone(), &primary_path).expect("open kv data");
    // Point the rebuild path at a directory that cannot exist, so
    // rebuild_secondary() fails deterministically.
    let broken = KvData {
      primary_path: Arc::new(temp.path().join("missing").join("primary")),
      ..kv_data
    };
    assert!(broken.rebuild_secondary().is_err());

    // The degraded read handle is the primary itself, and reads through it
    // still see the data.
    let fallback = broken.primary_fallback();
    assert!(Arc::ptr_eq(&fallback, &db));
    let cf = sm_data_cf(&fallback).expect("primary sm_data cf");
    assert_eq!(
      fallback.get_cf(&cf, b"alpha").expect("read via primary"),
      Some(b"one".to_vec())
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

  #[tokio::test]
  async fn read_persisted_membership_opens_tuned_read_only_db() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let group_id = "users";
    let node_id = NodeId::from("node-a");

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
    }

    let membership = read_persisted_membership_for_group(temp.path(), group_id)
      .expect("read persisted membership")
      .expect("membership from rocksdb");
    assert!(membership.membership().get_node(&node_id).is_some());
  }
}

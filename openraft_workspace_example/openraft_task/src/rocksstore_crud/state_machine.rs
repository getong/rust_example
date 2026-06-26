//! RocksDB-backed task queue state machine.

use std::{
  collections::{BTreeMap, HashSet, VecDeque},
  fs, io,
  io::Cursor,
  path::PathBuf,
  sync::Arc,
  time::{SystemTime, UNIX_EPOCH},
};

use futures::{Stream, TryStreamExt};
use openraft::{
  EntryPayload, OptionalSend, RaftSnapshotBuilder, StorageError,
  alias::{LogIdOf, SnapshotDataOf, SnapshotMetaOf, SnapshotOf, StoredMembershipOf},
  entry::RaftEntry,
  storage::{EntryResponder, RaftStateMachine},
  type_config::TypeConfigExt,
};
use rocksdb::DB;
use serde::{Deserialize, Serialize};

use super::TypeConfig;
use crate::types_kv::{QueueCommand, QueueResponse, TaskRecord, TaskResult, TaskStatus};

const LAST_APPLIED_LOG_KEY: &str = "last_applied_log";
const LAST_MEMBERSHIP_KEY: &str = "last_membership";
const PENDING_QUEUE_KEY: &str = "pending_queue";

#[derive(Debug, Clone)]
pub struct RocksStateMachine {
  db: Arc<DB>,
  snapshot_dir: PathBuf,
  /// In-memory pending task ID queue.  Kept in sync with RocksDB via `WriteBatch`.
  /// Stored here to avoid a full RocksDB read on every `apply` call.
  pending: VecDeque<String>,
  /// Set of IDs currently in `pending` for O(1) duplicate checks.
  pending_ids: HashSet<String>,
}

impl RocksStateMachine {
  pub(crate) async fn new(db: Arc<DB>, snapshot_dir: PathBuf) -> Result<Self, io::Error> {
    db.cf_handle("sm_meta")
      .ok_or_else(|| io::Error::other("column family `sm_meta` not found"))?;
    db.cf_handle("sm_data")
      .ok_or_else(|| io::Error::other("column family `sm_data` not found"))?;

    fs::create_dir_all(&snapshot_dir)?;

    // Warm the in-memory pending queue from persisted state.
    let pending: VecDeque<String> = {
      let cf = db.cf_handle("sm_meta").unwrap();
      db.get_cf(cf, PENDING_QUEUE_KEY)
        .map_err(io::Error::other)?
        .map(|bytes| {
          sonic_rs::from_slice::<VecDeque<String>>(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        })
        .transpose()?
        .unwrap_or_default()
    };
    let pending_ids: HashSet<String> = pending.iter().cloned().collect();

    Ok(Self {
      db,
      snapshot_dir,
      pending,
      pending_ids,
    })
  }

  fn cf_sm_meta(&self) -> &rocksdb::ColumnFamily {
    self
      .db
      .cf_handle("sm_meta")
      .expect("column family `sm_meta` exists")
  }

  fn cf_sm_data(&self) -> &rocksdb::ColumnFamily {
    self
      .db
      .cf_handle("sm_data")
      .expect("column family `sm_data` exists")
  }

  pub async fn task(&self, task_id: &str) -> Result<Option<TaskRecord>, io::Error> {
    let db = self.db.clone();
    let task_id = task_id.to_string();
    TypeConfig::spawn_blocking(move || {
      let cf = db
        .cf_handle("sm_data")
        .ok_or_else(|| io::Error::other("column family `sm_data` not found"))?;
      db.get_cf(cf, task_id.as_bytes())
        .map_err(|e| io::Error::other(e.to_string()))?
        .map(|bytes| decode_task(&bytes))
        .transpose()
    })
    .await?
  }

  pub async fn tasks(&self) -> Result<Vec<TaskRecord>, io::Error> {
    let db = self.db.clone();
    TypeConfig::spawn_blocking(move || {
      let cf = db
        .cf_handle("sm_data")
        .ok_or_else(|| io::Error::other("column family `sm_data` not found"))?;
      let mut tasks = Vec::new();
      for item in db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
        let (_, value) = item.map_err(|e| io::Error::other(e.to_string()))?;
        tasks.push(decode_task(&value)?);
      }
      tasks.sort_by(|a, b| a.task_id.cmp(&b.task_id));
      Ok(tasks)
    })
    .await?
  }

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

fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, StorageError<TypeConfig>> {
  sonic_rs::to_vec(value).map_err(|e| StorageError::write(TypeConfig::err_from_error(&e)))
}

fn deserialize<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, StorageError<TypeConfig>> {
  sonic_rs::from_slice(bytes).map_err(|e| StorageError::read(TypeConfig::err_from_error(&e)))
}

fn decode_task(bytes: &[u8]) -> Result<TaskRecord, io::Error> {
  sonic_rs::from_slice(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[derive(Serialize, Deserialize)]
struct SnapshotFile {
  meta: SnapshotMetaOf<TypeConfig>,
  pending: VecDeque<String>,
  tasks: Vec<(Vec<u8>, Vec<u8>)>,
}

impl RaftSnapshotBuilder<TypeConfig> for RocksStateMachine {
  #[tracing::instrument(level = "trace", skip(self))]
  async fn build_snapshot(&mut self) -> Result<SnapshotOf<TypeConfig>, io::Error> {
    let (last_applied_log, last_membership) = self.get_meta()?;
    // Use the in-memory pending queue — no extra RocksDB round-trip.
    let pending = self.pending.clone();

    let snapshot_idx: u64 = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_micros() as u64;
    let snapshot_id = if let Some(ref last) = last_applied_log {
      format!(
        "{}-{}-{}",
        last.committed_leader_id(),
        last.index(),
        snapshot_idx
      )
    } else {
      format!("--{}", snapshot_idx)
    };

    let meta = SnapshotMetaOf::<TypeConfig> {
      last_log_id: last_applied_log,
      last_membership,
      snapshot_id: snapshot_id.clone(),
    };

    let db = self.db.clone();
    let tasks =
      TypeConfig::spawn_blocking(move || -> Result<Vec<(Vec<u8>, Vec<u8>)>, io::Error> {
        let snapshot = db.snapshot();
        let cf_data = db
          .cf_handle("sm_data")
          .ok_or_else(|| io::Error::other("column family `sm_data` not found"))?;
        let mut snapshot_tasks = Vec::new();
        for item in snapshot.iterator_cf(cf_data, rocksdb::IteratorMode::Start) {
          let (key, value) = item.map_err(|e| io::Error::other(e.to_string()))?;
          snapshot_tasks.push((key.to_vec(), value.to_vec()));
        }
        Ok(snapshot_tasks)
      })
      .await??;

    let snapshot_file = SnapshotFile {
      meta: meta.clone(),
      pending,
      tasks,
    };
    let file_bytes = serialize(&snapshot_file).map_err(|e| {
      StorageError::<TypeConfig>::write_snapshot(
        Some(meta.signature()),
        TypeConfig::err_from_error(&e),
      )
    })?;

    let snapshot_path = self.snapshot_dir.join(&snapshot_id);
    fs::write(&snapshot_path, &file_bytes).map_err(|e| {
      StorageError::<TypeConfig>::write_snapshot(
        Some(meta.signature()),
        TypeConfig::err_from_error(&e),
      )
    })?;

    Ok(SnapshotOf::<TypeConfig> {
      meta,
      snapshot: Cursor::new(file_bytes),
    })
  }
}

impl RaftStateMachine<TypeConfig> for RocksStateMachine {
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
    let mut batch = rocksdb::WriteBatch::default();
    // Clone the in-memory pending state — avoids a RocksDB read on every apply.
    let mut pending = self.pending.clone();
    let mut pending_ids = self.pending_ids.clone();
    let mut staged_tasks = BTreeMap::new();
    let mut last_applied_log = None;
    let mut last_membership: Option<StoredMembershipOf<TypeConfig>> = None;
    let mut responses = Vec::new();

    while let Some((entry, responder)) = entries.try_next().await? {
      last_applied_log = Some(entry.log_id());

      let response = match entry.payload {
        EntryPayload::Blank => QueueResponse::None,
        EntryPayload::Normal(ref command) => self.apply_command(
          &mut batch,
          &mut staged_tasks,
          &mut pending,
          &mut pending_ids,
          command,
        )?,
        EntryPayload::Membership(ref mem) => {
          last_membership = Some(StoredMembershipOf::<TypeConfig>::new(
            Some(entry.log_id),
            mem.clone(),
          ));
          QueueResponse::None
        }
      };

      if let Some(responder) = responder {
        responses.push((responder, response));
      }
    }

    let cf_meta = self.cf_sm_meta();
    if let Some(ref log_id) = last_applied_log {
      batch.put_cf(cf_meta, LAST_APPLIED_LOG_KEY, serialize(log_id)?);
    }
    if let Some(ref membership) = last_membership {
      batch.put_cf(cf_meta, LAST_MEMBERSHIP_KEY, serialize(membership)?);
    }
    batch.put_cf(cf_meta, PENDING_QUEUE_KEY, serialize(&pending)?);

    self
      .db
      .write(batch)
      .map_err(|e| io::Error::other(e.to_string()))?;

    // Commit the updated in-memory state only after the durable write succeeds.
    self.pending = pending;
    self.pending_ids = pending_ids;

    for (responder, response) in responses {
      responder.send(response);
    }

    Ok(())
  }

  async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
    self.clone()
  }

  async fn begin_receiving_snapshot(&mut self) -> Result<SnapshotDataOf<TypeConfig>, io::Error> {
    Ok(Cursor::new(Vec::new()))
  }

  async fn install_snapshot(
    &mut self,
    meta: &SnapshotMetaOf<TypeConfig>,
    snapshot: SnapshotDataOf<TypeConfig>,
  ) -> Result<(), io::Error> {
    let raw_bytes = snapshot.into_inner();
    let snapshot_file: SnapshotFile = sonic_rs::from_slice(&raw_bytes)
      .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let new_pending: VecDeque<String> = snapshot_file.pending.clone();
    let pending_bytes = serialize(&snapshot_file.pending)
      .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let last_applied_bytes = meta
      .last_log_id
      .as_ref()
      .map(|log_id| {
        serialize(log_id).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
      })
      .transpose()?;
    let last_membership_bytes = serialize(&meta.last_membership)
      .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let db = self.db.clone();
    let snapshot_tasks = snapshot_file.tasks.clone();
    TypeConfig::spawn_blocking(move || -> Result<(), io::Error> {
      let cf_data = db
        .cf_handle("sm_data")
        .ok_or_else(|| io::Error::other("column family `sm_data` not found"))?;
      let cf_meta = db
        .cf_handle("sm_meta")
        .ok_or_else(|| io::Error::other("column family `sm_meta` not found"))?;
      let mut batch = rocksdb::WriteBatch::default();

      for item in db.iterator_cf(cf_data, rocksdb::IteratorMode::Start) {
        let (key, _) = item.map_err(|e| io::Error::other(e.to_string()))?;
        batch.delete_cf(cf_data, &key);
      }
      for (key, value) in snapshot_tasks {
        batch.put_cf(cf_data, &key, &value);
      }

      if let Some(bytes) = last_applied_bytes {
        batch.put_cf(cf_meta, LAST_APPLIED_LOG_KEY, bytes);
      }
      batch.put_cf(cf_meta, LAST_MEMBERSHIP_KEY, last_membership_bytes);
      batch.put_cf(cf_meta, PENDING_QUEUE_KEY, pending_bytes);

      db.write(batch)
        .map_err(|e| io::Error::other(e.to_string()))?;
      db.flush_wal(true)
        .map_err(|e| io::Error::other(e.to_string()))
    })
    .await??;

    // Write the raw snapshot bytes to disk — no re-serialization needed.
    fs::write(self.snapshot_dir.join(&meta.snapshot_id), &raw_bytes)?;

    // Update in-memory pending state to match the installed snapshot.
    self.pending_ids = new_pending.iter().cloned().collect();
    self.pending = new_pending;

    Ok(())
  }

  async fn get_current_snapshot(&mut self) -> Result<Option<SnapshotOf<TypeConfig>>, io::Error> {
    let mut latest_snapshot_id: Option<String> = None;
    for entry in fs::read_dir(&self.snapshot_dir)? {
      let entry = entry?;
      let path = entry.path();
      if !path.is_file() {
        continue;
      }
      if let Some(filename) = path.file_name().and_then(|name| name.to_str()) {
        if latest_snapshot_id
          .as_ref()
          .is_none_or(|current| filename > current.as_str())
        {
          latest_snapshot_id = Some(filename.to_string());
        }
      }
    }

    let Some(snapshot_id) = latest_snapshot_id else {
      return Ok(None);
    };
    let bytes = fs::read(self.snapshot_dir.join(snapshot_id))?;
    let snapshot_file: SnapshotFile =
      sonic_rs::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(Some(SnapshotOf::<TypeConfig> {
      meta: snapshot_file.meta,
      snapshot: Cursor::new(bytes),
    }))
  }
}

impl RocksStateMachine {
  fn apply_command(
    &self,
    batch: &mut rocksdb::WriteBatch,
    staged_tasks: &mut BTreeMap<String, TaskRecord>,
    pending: &mut VecDeque<String>,
    pending_ids: &mut HashSet<String>,
    command: &QueueCommand,
  ) -> Result<QueueResponse, io::Error> {
    match command {
      QueueCommand::Submit { task } => {
        let mut task = task.clone();
        task.status = TaskStatus::Pending;
        task.lock_by = None;
        task.result = None;
        batch.put_cf(
          self.cf_sm_data(),
          task.task_id.as_bytes(),
          serialize_task(&task)?,
        );
        staged_tasks.insert(task.task_id.clone(), task.clone());
        if pending_ids.insert(task.task_id.clone()) {
          pending.push_back(task.task_id.clone());
        }
        Ok(QueueResponse::submitted(task.task_id))
      }
      QueueCommand::SubmitBatch { tasks } => {
        let count = tasks.len();
        for task in tasks {
          let mut task = task.clone();
          task.status = TaskStatus::Pending;
          task.lock_by = None;
          task.result = None;
          batch.put_cf(
            self.cf_sm_data(),
            task.task_id.as_bytes(),
            serialize_task(&task)?,
          );
          staged_tasks.insert(task.task_id.clone(), task.clone());
          if pending_ids.insert(task.task_id.clone()) {
            pending.push_back(task.task_id.clone());
          }
        }
        Ok(QueueResponse::SubmittedBatch { count })
      }
      QueueCommand::Claim { worker_id, now } => {
        let claimed =
          self.claim_next(batch, staged_tasks, pending, pending_ids, worker_id, *now)?;
        Ok(QueueResponse::Claimed(claimed))
      }
      QueueCommand::Complete { task_id, result } => {
        if let Some(mut task) = self.load_task(staged_tasks, task_id)? {
          task.status = TaskStatus::Done;
          task.result = Some(TaskResult {
            ok: true,
            payload: result.clone(),
            error: None,
          });
          batch.put_cf(
            self.cf_sm_data(),
            task_id.as_bytes(),
            serialize_task(&task)?,
          );
          staged_tasks.insert(task_id.clone(), task);
        }
        Ok(QueueResponse::updated(task_id.clone()))
      }
      QueueCommand::Fail {
        task_id,
        reason,
        retry,
      } => {
        if let Some(mut task) = self.load_task(staged_tasks, task_id)? {
          task.result = Some(TaskResult {
            ok: false,
            payload: Vec::new(),
            error: Some(reason.clone()),
          });
          if *retry {
            task.status = TaskStatus::Pending;
            task.lock_by = None;
            task.claimed_at = None;
            if pending_ids.insert(task_id.clone()) {
              pending.push_back(task_id.clone());
            }
          } else {
            task.status = TaskStatus::Failed;
          }
          batch.put_cf(
            self.cf_sm_data(),
            task_id.as_bytes(),
            serialize_task(&task)?,
          );
          staged_tasks.insert(task_id.clone(), task);
        }
        Ok(QueueResponse::updated(task_id.clone()))
      }
      QueueCommand::Kill { task_id } => {
        if let Some(mut task) = self.load_task(staged_tasks, task_id)? {
          task.status = TaskStatus::Killed;
          task.lock_by = None;
          batch.put_cf(
            self.cf_sm_data(),
            task_id.as_bytes(),
            serialize_task(&task)?,
          );
          staged_tasks.insert(task_id.clone(), task);
        }
        pending.retain(|queued_id| queued_id != task_id);
        pending_ids.remove(task_id);
        Ok(QueueResponse::updated(task_id.clone()))
      }
      QueueCommand::Reclaim { timeout_secs, now } => {
        // Collect timed-out Running tasks with a full scan.
        // Staged tasks (modified earlier in this batch) take precedence over disk.
        let mut to_reclaim: Vec<TaskRecord> = Vec::new();
        {
          let cf = self.cf_sm_data();
          for item in self.db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
            let (key, value) = item.map_err(|e| io::Error::other(e.to_string()))?;
            let task_id = String::from_utf8_lossy(&key).to_string();
            let task = if let Some(t) = staged_tasks.get(&task_id) {
              t.clone()
            } else {
              decode_task(&value)?
            };
            if task.status != TaskStatus::Running {
              continue;
            }
            let timed_out = match task.claimed_at {
              Some(claimed_at) => now.saturating_sub(claimed_at) >= *timeout_secs,
              // Running without a claim timestamp — reclaim defensively.
              None => true,
            };
            if timed_out {
              to_reclaim.push(task);
            }
          }
        } // iterator dropped; batch writes are now safe

        let count = to_reclaim.len();
        for mut task in to_reclaim {
          task.status = TaskStatus::Pending;
          task.lock_by = None;
          task.claimed_at = None;
          batch.put_cf(
            self.cf_sm_data(),
            task.task_id.as_bytes(),
            serialize_task(&task)?,
          );
          if pending_ids.insert(task.task_id.clone()) {
            pending.push_back(task.task_id.clone());
          }
          staged_tasks.insert(task.task_id.clone(), task);
        }
        Ok(QueueResponse::Reclaimed { count })
      }
    }
  }

  fn claim_next(
    &self,
    batch: &mut rocksdb::WriteBatch,
    staged_tasks: &mut BTreeMap<String, TaskRecord>,
    pending: &mut VecDeque<String>,
    pending_ids: &mut HashSet<String>,
    worker_id: &str,
    now: u64,
  ) -> Result<Option<TaskRecord>, io::Error> {
    let mut skipped = VecDeque::new();

    while let Some(task_id) = pending.pop_front() {
      let Some(mut task) = self.load_task(staged_tasks, &task_id)? else {
        // Task record missing — drop from pending_ids too.
        pending_ids.remove(&task_id);
        continue;
      };
      if task.status != TaskStatus::Pending {
        pending_ids.remove(&task_id);
        continue;
      }
      if task.run_at > now {
        skipped.push_back(task_id);
        continue;
      }

      // Claim the task.
      pending_ids.remove(&task_id);
      task.status = TaskStatus::Running;
      task.lock_by = Some(worker_id.to_string());
      task.claimed_at = Some(now);
      task.attempts = task.attempts.saturating_add(1);
      batch.put_cf(
        self.cf_sm_data(),
        task.task_id.as_bytes(),
        serialize_task(&task)?,
      );
      staged_tasks.insert(task.task_id.clone(), task.clone());
      pending.extend(skipped);
      return Ok(Some(task));
    }

    pending.extend(skipped);
    Ok(None)
  }

  fn load_task(
    &self,
    staged_tasks: &BTreeMap<String, TaskRecord>,
    task_id: &str,
  ) -> Result<Option<TaskRecord>, io::Error> {
    if let Some(task) = staged_tasks.get(task_id) {
      return Ok(Some(task.clone()));
    }

    self
      .db
      .get_cf(self.cf_sm_data(), task_id.as_bytes())
      .map_err(|e| io::Error::other(e.to_string()))?
      .map(|bytes| decode_task(&bytes))
      .transpose()
  }
}

fn serialize_task(task: &TaskRecord) -> Result<Vec<u8>, io::Error> {
  sonic_rs::to_vec(task).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

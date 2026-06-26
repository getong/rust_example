//! RocksDB-backed task queue state machine.

use std::{
  collections::{BTreeMap, VecDeque},
  fs, io,
  io::Cursor,
  path::PathBuf,
  sync::Arc,
};

use futures::{Stream, TryStreamExt};
use openraft::{
  EntryPayload, OptionalSend, RaftSnapshotBuilder, StorageError,
  alias::{LogIdOf, SnapshotDataOf, SnapshotMetaOf, SnapshotOf, StoredMembershipOf},
  entry::RaftEntry,
  storage::{EntryResponder, RaftStateMachine},
  type_config::TypeConfigExt,
};
use rand::RngExt;
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
}

impl RocksStateMachine {
  pub(crate) async fn new(db: Arc<DB>, snapshot_dir: PathBuf) -> Result<Self, io::Error> {
    db.cf_handle("sm_meta")
      .ok_or_else(|| io::Error::other("column family `sm_meta` not found"))?;
    db.cf_handle("sm_data")
      .ok_or_else(|| io::Error::other("column family `sm_data` not found"))?;

    fs::create_dir_all(&snapshot_dir)?;
    Ok(Self { db, snapshot_dir })
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

  fn load_pending_queue(&self) -> Result<VecDeque<String>, StorageError<TypeConfig>> {
    self
      .db
      .get_cf(self.cf_sm_meta(), PENDING_QUEUE_KEY)
      .map_err(|e| StorageError::read(TypeConfig::err_from_error(&e)))?
      .map(|bytes| deserialize(&bytes))
      .transpose()
      .map(|queue| queue.unwrap_or_default())
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
    let pending = self.load_pending_queue()?;

    let snapshot_idx: u64 = rand::rng().random_range(0 .. 1000);
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
    let mut pending = self
      .load_pending_queue()
      .map_err(|e| io::Error::other(e.to_string()))?;
    let mut staged_tasks = BTreeMap::new();
    let mut last_applied_log = None;
    let mut last_membership: Option<StoredMembershipOf<TypeConfig>> = None;
    let mut responses = Vec::new();

    while let Some((entry, responder)) = entries.try_next().await? {
      last_applied_log = Some(entry.log_id());

      let response = match entry.payload {
        EntryPayload::Blank => QueueResponse::None,
        EntryPayload::Normal(ref command) => {
          self.apply_command(&mut batch, &mut staged_tasks, &mut pending, command)?
        }
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
    let snapshot_file: SnapshotFile = sonic_rs::from_slice(snapshot.get_ref())
      .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let snapshot_pending = snapshot_file.pending.clone();
    let snapshot_tasks = snapshot_file.tasks.clone();
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
      for (key, value) in snapshot_file.tasks {
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

    let snapshot_file = SnapshotFile {
      meta: meta.clone(),
      pending: snapshot_pending,
      tasks: snapshot_tasks,
    };
    let file_bytes = sonic_rs::to_vec(&snapshot_file)
      .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(self.snapshot_dir.join(&meta.snapshot_id), file_bytes)?;

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
        if !pending.contains(&task.task_id) {
          pending.push_back(task.task_id.clone());
        }
        Ok(QueueResponse::submitted(task.task_id))
      }
      QueueCommand::Claim { worker_id, now } => {
        let claimed = self.claim_next(batch, staged_tasks, pending, worker_id, *now)?;
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
            pending.push_back(task_id.clone());
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
        Ok(QueueResponse::updated(task_id.clone()))
      }
    }
  }

  fn claim_next(
    &self,
    batch: &mut rocksdb::WriteBatch,
    staged_tasks: &mut BTreeMap<String, TaskRecord>,
    pending: &mut VecDeque<String>,
    worker_id: &str,
    now: u64,
  ) -> Result<Option<TaskRecord>, io::Error> {
    let mut skipped = VecDeque::new();

    while let Some(task_id) = pending.pop_front() {
      let Some(mut task) = self.load_task(staged_tasks, &task_id)? else {
        continue;
      };
      if task.status != TaskStatus::Pending {
        continue;
      }
      if task.run_at > now {
        skipped.push_back(task_id);
        continue;
      }

      task.status = TaskStatus::Running;
      task.lock_by = Some(worker_id.to_string());
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

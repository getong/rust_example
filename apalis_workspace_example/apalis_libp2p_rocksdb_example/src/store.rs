use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use chrono::Utc;
use rocksdb::{ColumnFamily, ColumnFamilyDescriptor, DB, IteratorMode, Options, ReadOptions};
use serde::{Deserialize, Serialize};

use crate::model::{DistributedTask, TaskRecord, TaskStatus};

const TASKS_CF: &str = "tasks";
const META_CF: &str = "meta";

#[derive(Clone)]
pub struct TaskStore {
  db: Arc<DB>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeEvent {
  pub event: String,
  pub detail: String,
  pub created_at: i64,
}

impl TaskStore {
  pub fn open(path: impl AsRef<Path>) -> Result<Self> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let tasks = ColumnFamilyDescriptor::new(TASKS_CF, Options::default());
    let meta = ColumnFamilyDescriptor::new(META_CF, Options::default());

    let db = DB::open_cf_descriptors(&opts, path.as_ref(), vec![tasks, meta])
      .with_context(|| format!("opening RocksDB at {}", path.as_ref().display()))?;

    Ok(Self { db: Arc::new(db) })
  }

  pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self> {
    let opts = Options::default();
    let db = DB::open_cf_for_read_only(&opts, path.as_ref(), [TASKS_CF, META_CF], false)
      .with_context(|| format!("opening RocksDB read-only at {}", path.as_ref().display()))?;

    Ok(Self { db: Arc::new(db) })
  }

  pub fn put_status(
    &self,
    task: &DistributedTask,
    status: TaskStatus,
    node: Option<String>,
    output: Option<String>,
  ) -> Result<()> {
    let mut record = TaskRecord::new(task.clone(), status, node);
    record.output = output;
    self.put_record(&record)
  }

  pub fn update_with_output(
    &self,
    task: &DistributedTask,
    status: TaskStatus,
    node: Option<String>,
    output: impl Into<String>,
  ) -> Result<()> {
    self.put_status(task, status, node, Some(output.into()))
  }

  pub fn get(&self, task_id: &str) -> Result<Option<TaskRecord>> {
    let Some(value) = self
      .db
      .get_cf(self.tasks_cf()?, task_id.as_bytes())
      .with_context(|| format!("reading task {task_id}"))?
    else {
      return Ok(None);
    };

    serde_json::from_slice(&value)
      .with_context(|| format!("deserializing task record {task_id}"))
      .map(Some)
  }

  pub fn list_by_status(&self, status: TaskStatus) -> Result<Vec<TaskRecord>> {
    self.list_where(|record| record.status == status)
  }

  pub fn list_active(&self) -> Result<Vec<TaskRecord>> {
    self.list_where(|record| matches!(record.status, TaskStatus::Assigned | TaskStatus::Running))
  }

  pub fn list_all(&self) -> Result<Vec<TaskRecord>> {
    self.list_where(|_| true)
  }

  fn list_where(&self, predicate: impl Fn(&TaskRecord) -> bool) -> Result<Vec<TaskRecord>> {
    let mut read_opts = ReadOptions::default();
    read_opts.set_total_order_seek(true);
    let iter = self
      .db
      .iterator_cf_opt(self.tasks_cf()?, read_opts, IteratorMode::Start);

    let mut records = Vec::new();
    for item in iter {
      let (_, value) = item.context("reading task iterator item")?;
      let record: TaskRecord =
        serde_json::from_slice(&value).context("deserializing task record from iterator")?;
      if predicate(&record) {
        records.push(record);
      }
    }
    Ok(records)
  }

  pub fn append_event(&self, event: impl Into<String>, detail: impl Into<String>) -> Result<()> {
    let event = NodeEvent {
      event: event.into(),
      detail: detail.into(),
      created_at: Utc::now().timestamp_millis(),
    };
    let key = format!("event:{}", event.created_at);
    let value = serde_json::to_vec(&event).context("serializing node event")?;
    self
      .db
      .put_cf(self.meta_cf()?, key.as_bytes(), value)
      .context("writing node event")?;
    Ok(())
  }

  fn put_record(&self, record: &TaskRecord) -> Result<()> {
    let value = serde_json::to_vec(record).context("serializing task record")?;
    self
      .db
      .put_cf(self.tasks_cf()?, record.task.id.as_bytes(), value)
      .with_context(|| format!("writing task {}", record.task.id))?;
    self.db.flush_wal(true).context("flushing RocksDB WAL")?;
    Ok(())
  }

  fn tasks_cf(&self) -> Result<&ColumnFamily> {
    self
      .db
      .cf_handle(TASKS_CF)
      .context("missing tasks column family")
  }

  fn meta_cf(&self) -> Result<&ColumnFamily> {
    self
      .db
      .cf_handle(META_CF)
      .context("missing meta column family")
  }
}

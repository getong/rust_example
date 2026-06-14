use std::{
  fmt,
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, oneshot};

static GENERATED_TASK_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedTask {
  pub id: String,
  pub payload: String,
  pub created_at: i64,
}

impl DistributedTask {
  #[must_use]
  pub fn new(sequence: u64) -> Self {
    let now = Utc::now().timestamp_millis();
    Self {
      id: format!("task-{now}-{sequence}"),
      payload: format!("payload #{sequence}"),
      created_at: now,
    }
  }

  #[must_use]
  pub fn from_payload(payload: impl Into<String>) -> Self {
    let now = Utc::now().timestamp_millis();
    let sequence = GENERATED_TASK_COUNTER.fetch_add(1, Ordering::Relaxed);
    Self {
      id: format!("task-{now}-manual-{sequence}"),
      payload: payload.into(),
      created_at: now,
    }
  }

  #[must_use]
  pub fn with_id(id: impl Into<String>, payload: impl Into<String>) -> Self {
    Self {
      id: id.into(),
      payload: payload.into(),
      created_at: Utc::now().timestamp_millis(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskRequest {
  RegisterWorker,
  Submit { task: DistributedTask },
  Run { task: DistributedTask },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResponse {
  pub task_id: String,
  pub accepted: bool,
  pub output: String,
  pub worker: String,
  pub finished_at: i64,
}

impl TaskResponse {
  #[must_use]
  pub fn accepted(task_id: String, output: String, worker: String) -> Self {
    Self {
      task_id,
      accepted: true,
      output,
      worker,
      finished_at: Utc::now().timestamp_millis(),
    }
  }

  #[must_use]
  pub fn rejected(task_id: String, output: String, worker: String) -> Self {
    Self {
      task_id,
      accepted: false,
      output,
      worker,
      finished_at: Utc::now().timestamp_millis(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
  Created,
  Assigned,
  Received,
  Running,
  Completed,
  Failed,
}

impl fmt::Display for TaskStatus {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Created => f.write_str("created"),
      Self::Assigned => f.write_str("assigned"),
      Self::Received => f.write_str("received"),
      Self::Running => f.write_str("running"),
      Self::Completed => f.write_str("completed"),
      Self::Failed => f.write_str("failed"),
    }
  }
}

impl std::str::FromStr for TaskStatus {
  type Err = String;

  fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
    match value.to_ascii_lowercase().as_str() {
      "created" => Ok(Self::Created),
      "assigned" => Ok(Self::Assigned),
      "received" => Ok(Self::Received),
      "running" => Ok(Self::Running),
      "completed" => Ok(Self::Completed),
      "failed" => Ok(Self::Failed),
      _ => Err(format!(
        "unknown status {value}; expected one of: created, assigned, received, running, \
         completed, failed"
      )),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
  pub task: DistributedTask,
  pub status: TaskStatus,
  pub node: Option<String>,
  pub output: Option<String>,
  pub updated_at: i64,
}

impl TaskRecord {
  #[must_use]
  pub fn new(task: DistributedTask, status: TaskStatus, node: Option<String>) -> Self {
    Self {
      task,
      status,
      node,
      output: None,
      updated_at: Utc::now().timestamp_millis(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct WorkerJob {
  pub task: DistributedTask,
  pub reply: SharedReply,
}

#[derive(Debug, Clone)]
pub struct SharedReply(Arc<Mutex<Option<oneshot::Sender<TaskResponse>>>>);

impl SharedReply {
  #[must_use]
  pub fn new(sender: oneshot::Sender<TaskResponse>) -> Self {
    Self(Arc::new(Mutex::new(Some(sender))))
  }

  pub async fn send(&self, response: TaskResponse) -> bool {
    let sender = self.0.lock().await.take();
    sender.is_some_and(|sender| sender.send(response).is_ok())
  }
}

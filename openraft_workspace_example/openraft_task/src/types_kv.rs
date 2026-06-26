//! Raft replicated task queue commands and responses.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueueCommand {
  Submit {
    task: TaskRecord,
  },
  /// Batch-submit multiple tasks in a single Raft log entry.
  SubmitBatch {
    tasks: Vec<TaskRecord>,
  },
  Claim {
    worker_id: String,
    now: u64,
  },
  Complete {
    task_id: String,
    result: Vec<u8>,
  },
  Fail {
    task_id: String,
    reason: String,
    retry: bool,
  },
  Kill {
    task_id: String,
  },
}

impl fmt::Display for QueueCommand {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Submit { task } => write!(f, "submit({})", task.task_id),
      Self::SubmitBatch { tasks } => write!(f, "submit_batch(count={})", tasks.len()),
      Self::Claim { worker_id, .. } => write!(f, "claim({worker_id})"),
      Self::Complete { task_id, .. } => write!(f, "complete({task_id})"),
      Self::Fail { task_id, retry, .. } => write!(f, "fail({task_id}, retry={retry})"),
      Self::Kill { task_id } => write!(f, "kill({task_id})"),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
  pub task_id: String,
  pub payload: Vec<u8>,
  pub attempts: usize,
  pub status: TaskStatus,
  pub run_at: u64,
  pub lock_by: Option<String>,
  pub result: Option<TaskResult>,
}

impl TaskRecord {
  pub fn pending(task_id: impl Into<String>, payload: Vec<u8>, run_at: u64) -> Self {
    Self {
      task_id: task_id.into(),
      payload,
      attempts: 0,
      status: TaskStatus::Pending,
      run_at,
      lock_by: None,
      result: None,
    }
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
  Pending,
  Running,
  Done,
  Failed,
  Killed,
}

impl TaskStatus {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Pending => "pending",
      Self::Running => "running",
      Self::Done => "done",
      Self::Failed => "failed",
      Self::Killed => "killed",
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
  pub ok: bool,
  pub payload: Vec<u8>,
  pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueueResponse {
  Submitted { task_id: String },
  SubmittedBatch { count: usize },
  Claimed(Option<TaskRecord>),
  Updated { task_id: String },
  None,
}

impl QueueResponse {
  pub fn submitted(task_id: impl Into<String>) -> Self {
    Self::Submitted {
      task_id: task_id.into(),
    }
  }

  pub fn updated(task_id: impl Into<String>) -> Self {
    Self::Updated {
      task_id: task_id.into(),
    }
  }
}

//! Shared state-machine request/response types for this crate.
//!
//! Besides the generic KV commands, the state machine understands
//! task-native commands (octopii-style: a task is just a replicated
//! domain command). Task state transitions and their secondary indexes are
//! applied ATOMICALLY inside `apply()`; task EXECUTION never happens there —
//! workers claim tasks and run the side effects on exactly one node.
//!
//! Determinism rule: every timestamp in these commands is supplied by the
//! proposer. `apply()` must never read the clock.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A request to the replicated state machine.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Request {
  Set {
    key: String,
    value: String,
  },
  Delete {
    key: String,
  },

  /// Enqueue a task. Deduplicates on `idem_key` inside apply().
  TaskEnqueue {
    id: String,
    /// Serialized task arguments (JSON), opaque to the state machine.
    payload: String,
    /// Unix seconds after which the task may be scheduled.
    run_at: u64,
    idem_key: Option<String>,
  },
  /// Leader schedules a queued task to a worker (moves queued → assigned).
  TaskAssign {
    id: String,
    node_id: String,
    lease_epoch: u64,
  },
  /// Worker atomically claims its assigned task (assigned → running).
  /// Succeeds only when (node_id, lease_epoch) match the record.
  TaskClaim {
    id: String,
    node_id: String,
    lease_epoch: u64,
  },
  /// Worker reports success (running → done). Stale acks are rejected.
  TaskDone {
    id: String,
    node_id: String,
    lease_epoch: u64,
    attempts: u32,
  },
  /// Worker reports failure. `retry_at > 0` re-queues with that run_at
  /// (running → queued); `retry_at == 0` marks the task failed permanently.
  TaskFail {
    id: String,
    node_id: String,
    lease_epoch: u64,
    attempts: u32,
    error: String,
    retry_at: u64,
  },
  /// Leader returns an assigned/running task of an inactive worker to the
  /// queue.
  TaskRequeue {
    id: String,
  },
  /// Worker lease heartbeat record.
  WorkerLease {
    node_id: String,
    worker_name: String,
    lease_epoch: u64,
    expires_at: u64,
  },
}

impl Request {
  pub fn set(key: impl Into<String>, value: impl Into<String>) -> Self {
    Request::Set {
      key: key.into(),
      value: value.into(),
    }
  }

  pub fn delete(key: impl Into<String>) -> Self {
    Request::Delete { key: key.into() }
  }
}

impl fmt::Display for Request {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Request::Set { key, value } => write!(f, "Set {{ key: {}, value: {} }}", key, value),
      Request::Delete { key } => write!(f, "Delete {{ key: {} }}", key),
      Request::TaskEnqueue { id, .. } => write!(f, "TaskEnqueue {{ id: {id} }}"),
      Request::TaskAssign { id, node_id, .. } => {
        write!(f, "TaskAssign {{ id: {id}, node: {node_id} }}")
      }
      Request::TaskClaim { id, node_id, .. } => {
        write!(f, "TaskClaim {{ id: {id}, node: {node_id} }}")
      }
      Request::TaskDone { id, .. } => write!(f, "TaskDone {{ id: {id} }}"),
      Request::TaskFail { id, .. } => write!(f, "TaskFail {{ id: {id} }}"),
      Request::TaskRequeue { id } => write!(f, "TaskRequeue {{ id: {id} }}"),
      Request::WorkerLease { node_id, .. } => write!(f, "WorkerLease {{ node: {node_id} }}"),
    }
  }
}

/// A response from the KV store.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
  pub value: Option<String>,
}

impl Response {
  pub fn new(value: impl Into<String>) -> Self {
    Response {
      value: Some(value.into()),
    }
  }

  pub fn none() -> Self {
    Response { value: None }
  }
}

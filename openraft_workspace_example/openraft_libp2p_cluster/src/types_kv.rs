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

/// A request to the replicated state machine: the generic KV commands plus
/// the task domain, kept as a SEPARATE enum ([`TaskRequest`]) so only the
/// task subsystem deals with task commands.
///
/// `#[serde(untagged)]` on the `Task` variant erases the wrapper on the
/// wire: `Request::Task(TaskRequest::TaskEnqueue { .. })` still serializes
/// as `{"TaskEnqueue":{..}}`, byte-identical to the historical flat enum,
/// so raft log entries written before the split keep decoding.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Request {
  Set {
    key: String,
    value: String,
  },
  Delete {
    key: String,
  },
  #[serde(untagged)]
  Task(TaskRequest),
}

impl From<TaskRequest> for Request {
  fn from(task: TaskRequest) -> Self {
    Request::Task(task)
  }
}

/// Task-domain commands of the replicated state machine (octopii-style: a
/// task transition is just a replicated domain command). Only the "tasks"
/// raft group ever carries these.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TaskRequest {
  /// Enqueue a task. Deduplicates on `idem_key` inside apply().
  TaskEnqueue {
    id: String,
    /// Serialized task arguments (JSON), opaque to the state machine.
    payload: String,
    /// Unix seconds after which the task may be scheduled.
    run_at: u64,
    idem_key: Option<String>,
    /// Submission time (proposer-supplied), stamped as `created_at`.
    /// `serde(default)` keeps older log entries decodable.
    #[serde(default)]
    created_at: u64,
  },
  /// Leader schedules a queued task to a worker (moves queued → assigned).
  /// `now` (proposer-supplied) stamps the record's `updated_at` for
  /// stuck-task detection.
  TaskAssign {
    id: String,
    node_id: String,
    lease_epoch: u64,
    now: u64,
  },
  /// Worker atomically claims its assigned task (assigned → running).
  /// Succeeds only when (node_id, lease_epoch) match the record.
  TaskClaim {
    id: String,
    node_id: String,
    lease_epoch: u64,
    now: u64,
  },
  /// Worker marks its running task as past the point of no return, BEFORE
  /// performing an irreversible side effect (renegade's `Running.committed`
  /// pattern). Fenced like an ack: succeeds only when (node_id, lease_epoch)
  /// match the record. Once committed, `TaskRequeue` refuses to re-queue the
  /// task and fails it terminally instead — a re-run could duplicate the
  /// side effect. The worker must abort the side effect if this command is
  /// rejected: winning this raft write is what resolves the race against a
  /// concurrent requeue.
  TaskMarkCommitted {
    id: String,
    node_id: String,
    lease_epoch: u64,
    /// Commit time (proposer-supplied), stamped as `updated_at`.
    #[serde(default)]
    now: u64,
  },
  /// Worker reports success (running → done). Stale acks are rejected.
  TaskDone {
    id: String,
    node_id: String,
    lease_epoch: u64,
    attempts: u32,
    /// Completion time (proposer-supplied), stamped as `completed_at`.
    #[serde(default)]
    now: u64,
    /// Execution result produced by the handler (opaque JSON), stored on
    /// the record until vacuumed.
    #[serde(default)]
    result: Option<String>,
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
    /// Failure time (proposer-supplied); stamps `completed_at` on
    /// permanent failure and `updated_at` on retry.
    #[serde(default)]
    now: u64,
  },
  /// Leader returns an assigned/running task of an inactive worker to the
  /// queue.
  TaskRequeue { id: String },
  /// Operator-driven dead-letter replay: return a permanently FAILED task
  /// to the queue with a fresh attempt budget. Committed tasks are refused —
  /// their side effect may already have executed, so a re-run could
  /// duplicate it (reconcile instead).
  TaskReplay {
    id: String,
    /// Replay time (proposer-supplied): stamps `updated_at` and becomes the
    /// new `run_at`, so the task is due immediately.
    #[serde(default)]
    now: u64,
  },
  /// Leader-driven retention cleanup: delete the listed TERMINAL
  /// (done/failed) task records, their terminal-index entries, and their
  /// idempotency keys. The leader picks the ids OUTSIDE apply (scan of the
  /// terminal index against the retention cutoff); apply only re-validates
  /// per id, keeping the command deterministic on every replica.
  TaskVacuum { ids: Vec<String> },
  /// Worker lease heartbeat record.
  WorkerLease {
    node_id: String,
    worker_name: String,
    lease_epoch: u64,
    expires_at: u64,
  },
}

impl fmt::Display for TaskRequest {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      TaskRequest::TaskEnqueue { id, .. } => write!(f, "TaskEnqueue {{ id: {id} }}"),
      TaskRequest::TaskAssign { id, node_id, .. } => {
        write!(f, "TaskAssign {{ id: {id}, node: {node_id} }}")
      }
      TaskRequest::TaskClaim { id, node_id, .. } => {
        write!(f, "TaskClaim {{ id: {id}, node: {node_id} }}")
      }
      TaskRequest::TaskMarkCommitted { id, node_id, .. } => {
        write!(f, "TaskMarkCommitted {{ id: {id}, node: {node_id} }}")
      }
      TaskRequest::TaskDone { id, .. } => write!(f, "TaskDone {{ id: {id} }}"),
      TaskRequest::TaskFail { id, .. } => write!(f, "TaskFail {{ id: {id} }}"),
      TaskRequest::TaskRequeue { id } => write!(f, "TaskRequeue {{ id: {id} }}"),
      TaskRequest::TaskReplay { id, .. } => write!(f, "TaskReplay {{ id: {id} }}"),
      TaskRequest::TaskVacuum { ids } => write!(f, "TaskVacuum {{ ids: {} }}", ids.len()),
      TaskRequest::WorkerLease { node_id, .. } => write!(f, "WorkerLease {{ node: {node_id} }}"),
    }
  }
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
      Request::Task(task) => fmt::Display::fmt(task, f),
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

#[cfg(test)]
mod tests {
  use super::*;

  /// The `Task` wrapper must be invisible on the wire: log entries written
  /// before the enum split used the flat externally-tagged form, and the
  /// nested enum must keep both encoding and decoding byte-identical.
  #[test]
  fn task_variant_serializes_flat() {
    let request = Request::Task(TaskRequest::TaskRequeue {
      id: "t1".to_string(),
    });
    let json = sonic_rs::to_string(&request).expect("encode");
    assert_eq!(json, r#"{"TaskRequeue":{"id":"t1"}}"#);

    let decoded: Request = sonic_rs::from_str(&json).expect("decode");
    match decoded {
      Request::Task(TaskRequest::TaskRequeue { id }) => assert_eq!(id, "t1"),
      other => panic!("expected TaskRequeue, got {other:?}"),
    }
  }

  #[test]
  fn kv_variants_unchanged() {
    let json = sonic_rs::to_string(&Request::set("k", "v")).expect("encode");
    assert_eq!(json, r#"{"Set":{"key":"k","value":"v"}}"#);
    let decoded: Request = sonic_rs::from_str(&json).expect("decode");
    assert!(matches!(decoded, Request::Set { .. }));
  }

  /// A pre-split log entry (flat WorkerLease) decodes into the nested form.
  #[test]
  fn legacy_flat_task_entry_decodes() {
    let legacy =
      r#"{"WorkerLease":{"node_id":"n1","worker_name":"w","lease_epoch":3,"expires_at":99}}"#;
    let decoded: Request = sonic_rs::from_str(legacy).expect("decode legacy");
    match decoded {
      Request::Task(TaskRequest::WorkerLease {
        node_id,
        lease_epoch,
        ..
      }) => {
        assert_eq!(node_id, "n1");
        assert_eq!(lease_epoch, 3);
      }
      other => panic!("expected WorkerLease, got {other:?}"),
    }
  }
}

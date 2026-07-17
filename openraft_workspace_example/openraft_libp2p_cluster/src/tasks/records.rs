//! Data model of the task domain: the replicated records, the structured
//! apply result carried back to proposers, and the pure metrics aggregation
//! over them. Nothing here touches storage or the clock — records are plain
//! serializable state, and `compute_metrics` takes `now` from the caller.

use serde::{Deserialize, Serialize};

use crate::types_kv::Response;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
  Queued,
  Assigned,
  Running,
  Done,
  Failed,
}

impl TaskStatus {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Queued => "queued",
      Self::Assigned => "assigned",
      Self::Running => "running",
      Self::Done => "done",
      Self::Failed => "failed",
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
  pub id: String,
  /// Serialized task arguments (opaque JSON).
  pub payload: String,
  pub status: TaskStatus,
  pub attempts: u32,
  pub run_at: u64,
  pub idem_key: Option<String>,
  pub assigned_node_id: Option<String>,
  pub lease_epoch: Option<u64>,
  /// True once the executing worker declared the task past its point of no
  /// return (irreversible side effect about to run / in flight). A committed
  /// task is never re-queued: `TaskRequeue` fails it terminally instead,
  /// because a re-run could duplicate the side effect. Set by
  /// `TaskMarkCommitted` under the same (node, lease_epoch) fencing as acks.
  #[serde(default)]
  pub committed: bool,
  pub error: Option<String>,
  /// Unix seconds of the last assign/claim transition; the scheduler uses
  /// it to requeue tasks stuck in Assigned/Running on a live worker.
  #[serde(default)]
  pub updated_at: u64,
  /// Submission time (0 for records written before this field existed).
  #[serde(default)]
  pub created_at: u64,
  /// Time the task reached a terminal state (done / permanently failed);
  /// 0 while non-terminal.
  #[serde(default)]
  pub completed_at: u64,
  /// Handler-produced execution result (opaque JSON), set on success.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerLeaseRecord {
  pub node_id: String,
  pub worker_name: String,
  pub lease_epoch: u64,
  pub expires_at: u64,
}

/// Structured apply result carried back to the proposer in
/// `Response::value` as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOpResult {
  pub ok: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub deduplicated: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub record: Option<TaskRecord>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
}

impl TaskOpResult {
  pub(crate) fn ok() -> Self {
    Self {
      ok: true,
      id: None,
      deduplicated: None,
      record: None,
      reason: None,
    }
  }

  pub(crate) fn rejected(reason: impl Into<String>) -> Self {
    Self {
      ok: false,
      id: None,
      deduplicated: None,
      record: None,
      reason: Some(reason.into()),
    }
  }

  pub(crate) fn into_response(self) -> Response {
    Response {
      value: sonic_rs::to_string(&self).ok(),
    }
  }

  pub fn from_response(response: &Response) -> Option<Self> {
    sonic_rs::from_str(response.value.as_deref()?).ok()
  }
}

/// Point-in-time queue health snapshot served by `/tasks/metrics` and the
/// TaskRpc `metrics` method.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskQueueMetrics {
  pub total: usize,
  pub queued: usize,
  pub assigned: usize,
  pub running: usize,
  pub done: usize,
  pub failed: usize,
  /// Total execution attempts across all tasks.
  pub total_attempts: u64,
  /// Tasks that needed more than one attempt.
  pub retried_tasks: usize,
  /// Age in seconds of the oldest DUE queued task (0 when the queue is
  /// drained or nothing is due yet).
  pub oldest_due_queued_age_secs: u64,
  /// Worker leases valid at `computed_at`.
  pub active_workers: usize,
  pub total_worker_leases: usize,
  /// Average enqueue→terminal latency in seconds over completed tasks that
  /// carry both timestamps (0 when none do).
  pub avg_completion_latency_secs: u64,
  /// Maximum enqueue→terminal latency in seconds over the same set.
  pub max_completion_latency_secs: u64,
  /// Unix seconds when this snapshot was computed.
  pub computed_at: u64,
}

/// Pure aggregation over the current records; `now` supplied by the caller.
pub fn compute_metrics(
  records: &[TaskRecord],
  leases: &[WorkerLeaseRecord],
  now: u64,
) -> TaskQueueMetrics {
  let mut metrics = TaskQueueMetrics {
    total: records.len(),
    total_worker_leases: leases.len(),
    active_workers: leases.iter().filter(|l| l.expires_at >= now).count(),
    computed_at: now,
    ..TaskQueueMetrics::default()
  };

  for record in records {
    match record.status {
      TaskStatus::Queued => {
        metrics.queued += 1;
        if record.run_at <= now {
          let age = now.saturating_sub(record.run_at);
          metrics.oldest_due_queued_age_secs = metrics.oldest_due_queued_age_secs.max(age);
        }
      }
      TaskStatus::Assigned => metrics.assigned += 1,
      TaskStatus::Running => metrics.running += 1,
      TaskStatus::Done => metrics.done += 1,
      TaskStatus::Failed => metrics.failed += 1,
    }
    metrics.total_attempts += u64::from(record.attempts);
    if record.attempts > 1 {
      metrics.retried_tasks += 1;
    }
  }

  // Completion latency over terminal records with both timestamps (records
  // written before the timestamp fields existed report 0 and are skipped).
  let mut latency_sum = 0u64;
  let mut latency_count = 0u64;
  for record in records {
    if matches!(record.status, TaskStatus::Done | TaskStatus::Failed)
      && record.created_at > 0
      && record.completed_at >= record.created_at
    {
      let latency = record.completed_at - record.created_at;
      latency_sum += latency;
      latency_count += 1;
      metrics.max_completion_latency_secs = metrics.max_completion_latency_secs.max(latency);
    }
  }
  if latency_count > 0 {
    metrics.avg_completion_latency_secs = latency_sum / latency_count;
  }

  metrics
}

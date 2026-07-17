//! Task HTTP endpoints: thin adapters over [`crate::tasks::api::TaskApi`].
//!
//! All task-domain logic (validation, id/timestamps, control-vs-worker
//! dispatch, leader following) lives in the facade; this module only maps
//! HTTP request/response shapes.

use std::sync::Arc;

use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};

use super::{AppState, Json};
use crate::tasks::{
  TaskOpResult, TaskQueueMetrics, TaskRecord, WorkerLeaseRecord,
  handlers::{Email, TaskPayload},
};

#[derive(Deserialize)]
pub(super) struct EmailRequest {
  to: String,
  /// Optional idempotency key: repeated pushes with the same key return the
  /// original task id with `deduplicated: true`.
  idem_key: Option<String>,
}

/// Generic task submission: `payload` is the kind-tagged JSON handed to the
/// worker-side handler registry (e.g. `{"kind":"digest","data":"x"}`).
#[derive(Deserialize)]
pub(super) struct PushTaskRequest {
  payload: sonic_rs::Value,
  idem_key: Option<String>,
  /// Schedule the task `delay_secs` into the future (run_at = now + delay).
  #[serde(default)]
  delay_secs: u64,
}

#[derive(Serialize)]
pub(super) struct EmailResponse {
  ok: bool,
  task_id: Option<String>,
  deduplicated: Option<bool>,
  error: Option<String>,
}

#[derive(Serialize)]
pub(super) struct TasksResponse {
  ok: bool,
  tasks: Vec<TaskRecord>,
  error: Option<String>,
}

#[derive(Serialize)]
pub(super) struct TaskWorkersResponse {
  ok: bool,
  workers: Vec<WorkerLeaseRecord>,
  error: Option<String>,
}

#[derive(Serialize)]
pub(super) struct TaskMetricsResponse {
  ok: bool,
  metrics: Option<TaskQueueMetrics>,
  error: Option<String>,
}

#[derive(Serialize)]
pub(super) struct ReplayResponse {
  ok: bool,
  task_id: String,
  error: Option<String>,
}

fn push_error(message: String) -> EmailResponse {
  EmailResponse {
    ok: false,
    task_id: None,
    deduplicated: None,
    error: Some(message),
  }
}

fn push_response(outcome: anyhow::Result<TaskOpResult>) -> EmailResponse {
  match outcome {
    Ok(result) => EmailResponse {
      ok: result.ok,
      task_id: result.id,
      deduplicated: result.deduplicated,
      error: result.reason,
    },
    Err(err) => push_error(err.to_string()),
  }
}

pub(super) async fn push_email(
  State(state): State<Arc<AppState>>,
  Json(req): Json<EmailRequest>,
) -> Json<EmailResponse> {
  let payload = match TaskPayload::Email(Email { to: req.to }).encode() {
    Ok(payload) => payload,
    Err(err) => return Json(push_error(err)),
  };
  Json(push_response(
    state.task_api.enqueue(payload, req.idem_key, 0).await,
  ))
}

/// Generic multi-kind submission. The facade decodes the payload into a
/// [`TaskPayload`] variant, so an unknown or malformed kind is rejected at
/// submit time (not at execution).
pub(super) async fn push_task(
  State(state): State<Arc<AppState>>,
  Json(req): Json<PushTaskRequest>,
) -> Json<EmailResponse> {
  let payload = match sonic_rs::to_string(&req.payload) {
    Ok(payload) => payload,
    Err(err) => return Json(push_error(format!("encode task payload: {err}"))),
  };
  Json(push_response(
    state
      .task_api
      .enqueue(payload, req.idem_key, req.delay_secs)
      .await,
  ))
}

/// Dead-letter replay (`POST /tasks/{id}/replay`): return a permanently
/// failed task to the queue with a fresh attempt budget. The rules live in
/// the state machine (Failed only; committed tasks refused because their
/// side effect may have executed), so this endpoint just relays the verdict.
pub(super) async fn replay_task(
  State(state): State<Arc<AppState>>,
  Path(id): Path<String>,
) -> Json<ReplayResponse> {
  let (ok, error) = match state.task_api.replay(id.clone()).await {
    Ok(result) if result.ok => (true, None),
    Ok(result) => (false, result.reason),
    Err(err) => (false, Some(err.to_string())),
  };
  Json(ReplayResponse {
    ok,
    task_id: id,
    error,
  })
}

pub(super) async fn list_tasks(State(state): State<Arc<AppState>>) -> Json<TasksResponse> {
  Json(match state.task_api.list_tasks().await {
    Ok(tasks) => TasksResponse {
      ok: true,
      tasks,
      error: None,
    },
    Err(err) => TasksResponse {
      ok: false,
      tasks: Vec::new(),
      error: Some(err.to_string()),
    },
  })
}

pub(super) async fn list_task_workers(
  State(state): State<Arc<AppState>>,
) -> Json<TaskWorkersResponse> {
  Json(match state.task_api.list_workers().await {
    Ok(workers) => TaskWorkersResponse {
      ok: true,
      workers,
      error: None,
    },
    Err(err) => TaskWorkersResponse {
      ok: false,
      workers: Vec::new(),
      error: Some(err.to_string()),
    },
  })
}

pub(super) async fn task_metrics(State(state): State<Arc<AppState>>) -> Json<TaskMetricsResponse> {
  Json(match state.task_api.metrics().await {
    Ok(metrics) => TaskMetricsResponse {
      ok: true,
      metrics: Some(metrics),
      error: None,
    },
    Err(err) => TaskMetricsResponse {
      ok: false,
      metrics: None,
      error: Some(err.to_string()),
    },
  })
}

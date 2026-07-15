use std::{
  sync::Arc,
  time::{SystemTime, UNIX_EPOCH},
};

use axum::extract::State;
use serde::{Deserialize, Serialize};

use super::{AppState, Json, TaskFrontend};
use crate::{
  NodeId, groups,
  tasks::{
    TaskOpResult, TaskQueueMetrics, TaskRecord, WorkerLeaseRecord,
    handlers::{Email, TaskPayload},
    rpc::{ControlNodes, TaskRpc, TaskRpcRequest, TaskRpcResponse, TaskRpcService},
    worker::{call_read, submit_command},
  },
  types_kv::TaskRequest as StateCommand,
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

fn unix_now_secs() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs()
}

/// Submit a task state-machine command via the node's task frontend:
/// control nodes write through their local raft handle (following a leader
/// hint over the network when needed), workers go through the tarpc TaskRpc.
async fn submit_task_state_command(
  state: &AppState,
  cmd: StateCommand,
) -> anyhow::Result<TaskOpResult> {
  let group_id = groups::TASKS.to_string();
  match &state.task_frontend {
    TaskFrontend::Control => {
      let reply = TaskRpcService
        .submit(
          tarpc::context::current(),
          group_id.clone(),
          cmd.clone().into(),
        )
        .await;
      if reply.ok {
        let value = reply
          .value
          .ok_or_else(|| anyhow::anyhow!("task command accepted but returned no result"))?;
        return sonic_rs::from_str(&value)
          .map_err(|err| anyhow::anyhow!("decode task op result: {err}"));
      }
      // Not the leader for this group: follow the hint over the network.
      if let Some(leader_id) = reply.leader_id.as_deref() {
        let leader = NodeId::new(leader_id);
        if let Some(addr) = reply.leader_addr.as_deref() {
          let _ = state.network.register_node(leader.clone(), addr).await;
        }
        let control_nodes = tokio::sync::Mutex::new(ControlNodes::new(vec![leader]));
        return submit_command(&state.network, &control_nodes, &group_id, cmd).await;
      }
      Err(anyhow::anyhow!(
        "task command failed: {}",
        reply.error.unwrap_or_default()
      ))
    }
    TaskFrontend::Worker { control_nodes } => {
      submit_command(&state.network, control_nodes, &group_id, cmd).await
    }
  }
}

/// Enqueue one task with an already-encoded (kind-tagged) payload.
async fn enqueue_task(
  state: &AppState,
  payload: String,
  idem_key: Option<String>,
  delay_secs: u64,
) -> EmailResponse {
  if payload.len() > crate::tasks::MAX_TASK_PAYLOAD_BYTES {
    return push_error(format!(
      "task payload is {} bytes, over the {} byte limit (wasm modules must fit the raft log)",
      payload.len(),
      crate::tasks::MAX_TASK_PAYLOAD_BYTES
    ));
  }
  let now = unix_now_secs();
  let cmd = StateCommand::TaskEnqueue {
    id: uuid::Uuid::now_v7().to_string(),
    payload,
    run_at: now + delay_secs,
    idem_key,
    created_at: now,
  };

  match submit_task_state_command(state, cmd).await {
    Ok(result) if result.ok => EmailResponse {
      ok: true,
      task_id: result.id,
      deduplicated: result.deduplicated,
      error: None,
    },
    Ok(result) => EmailResponse {
      ok: false,
      task_id: result.id,
      deduplicated: result.deduplicated,
      error: result.reason,
    },
    Err(err) => EmailResponse {
      ok: false,
      task_id: None,
      deduplicated: None,
      error: Some(err.to_string()),
    },
  }
}

fn push_error(message: String) -> EmailResponse {
  EmailResponse {
    ok: false,
    task_id: None,
    deduplicated: None,
    error: Some(message),
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
  Json(enqueue_task(&state, payload, req.idem_key, 0).await)
}

/// Generic multi-kind submission. The payload must decode into a
/// [`TaskPayload`] variant, so an unknown or malformed kind is rejected here
/// at submit time (not at execution).
pub(super) async fn push_task(
  State(state): State<Arc<AppState>>,
  Json(req): Json<PushTaskRequest>,
) -> Json<EmailResponse> {
  let payload = match sonic_rs::to_string(&req.payload) {
    Ok(payload) => payload,
    Err(err) => return Json(push_error(format!("encode task payload: {err}"))),
  };
  if let Err(err) = TaskPayload::decode(&payload) {
    return Json(push_error(err));
  }
  Json(enqueue_task(&state, payload, req.idem_key, req.delay_secs).await)
}

pub(super) async fn list_tasks(State(state): State<Arc<AppState>>) -> Json<TasksResponse> {
  let group_id = groups::TASKS.to_string();
  let reply = match &state.task_frontend {
    TaskFrontend::Control => {
      TaskRpcService
        .list_tasks(tarpc::context::current(), group_id)
        .await
    }
    TaskFrontend::Worker { control_nodes } => {
      match call_read(&state.network, control_nodes, || {
        TaskRpcRequest::ListTasks {
          group_id: group_id.clone(),
        }
      })
      .await
      {
        Ok(TaskRpcResponse::ListTasks(reply)) => reply,
        Ok(other) => {
          return Json(TasksResponse {
            ok: false,
            tasks: Vec::new(),
            error: Some(format!("unexpected task rpc response: {other:?}")),
          });
        }
        Err(err) => {
          return Json(TasksResponse {
            ok: false,
            tasks: Vec::new(),
            error: Some(err.to_string()),
          });
        }
      }
    }
  };

  Json(TasksResponse {
    ok: reply.ok,
    tasks: reply.tasks,
    error: reply.error,
  })
}

pub(super) async fn list_task_workers(
  State(state): State<Arc<AppState>>,
) -> Json<TaskWorkersResponse> {
  let group_id = groups::TASKS.to_string();
  let reply = match &state.task_frontend {
    TaskFrontend::Control => {
      TaskRpcService
        .list_workers(tarpc::context::current(), group_id)
        .await
    }
    TaskFrontend::Worker { control_nodes } => {
      match call_read(&state.network, control_nodes, || {
        TaskRpcRequest::ListWorkers {
          group_id: group_id.clone(),
        }
      })
      .await
      {
        Ok(TaskRpcResponse::ListWorkers(reply)) => reply,
        Ok(other) => {
          return Json(TaskWorkersResponse {
            ok: false,
            workers: Vec::new(),
            error: Some(format!("unexpected task rpc response: {other:?}")),
          });
        }
        Err(err) => {
          return Json(TaskWorkersResponse {
            ok: false,
            workers: Vec::new(),
            error: Some(err.to_string()),
          });
        }
      }
    }
  };

  Json(TaskWorkersResponse {
    ok: reply.ok,
    workers: reply.workers,
    error: reply.error,
  })
}

pub(super) async fn task_metrics(State(state): State<Arc<AppState>>) -> Json<TaskMetricsResponse> {
  let group_id = groups::TASKS.to_string();
  let reply = match &state.task_frontend {
    TaskFrontend::Control => {
      TaskRpcService
        .metrics(tarpc::context::current(), group_id)
        .await
    }
    TaskFrontend::Worker { control_nodes } => {
      match call_read(&state.network, control_nodes, || TaskRpcRequest::Metrics {
        group_id: group_id.clone(),
      })
      .await
      {
        Ok(TaskRpcResponse::Metrics(reply)) => reply,
        Ok(other) => {
          return Json(TaskMetricsResponse {
            ok: false,
            metrics: None,
            error: Some(format!("unexpected task rpc response: {other:?}")),
          });
        }
        Err(err) => {
          return Json(TaskMetricsResponse {
            ok: false,
            metrics: None,
            error: Some(err.to_string()),
          });
        }
      }
    }
  };

  Json(TaskMetricsResponse {
    ok: reply.ok,
    metrics: reply.metrics,
    error: reply.error,
  })
}

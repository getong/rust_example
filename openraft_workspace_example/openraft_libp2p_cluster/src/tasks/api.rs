//! The unified task-domain facade (octopii's `OctopiiNode` pattern): ONE
//! entry point for everything a frontend does with the task queue —
//! enqueue / replay / list / metrics — regardless of whether this node is a
//! control node (local raft handle) or a worker (tarpc TaskRpc to control
//! nodes).
//!
//! Before this facade existed every consumer re-implemented the plumbing:
//! the HTTP layer carried its own control/worker dispatch, its own
//! leader-hint follow, and its own payload validation, duplicating the
//! logic in [`crate::tasks::worker`] and [`crate::tasks::rpc`]. All of that
//! now lives here exactly once:
//!
//!   - [`TaskApi::enqueue`] is the single enqueue door: payload size cap, kind-tag decode
//!     validation, id generation and timestamps happen here and nowhere else.
//!   - [`TaskApi::submit`] is the single write path: control nodes write through the local raft
//!     handle and follow a leader hint over the network when they are not the leader; workers
//!     submit via the TaskRpc client with its own leader stickiness.
//!   - The read methods ([`TaskApi::list_tasks`] / [`TaskApi::list_workers`] /
//!     [`TaskApi::metrics`]) hide the same control/worker split behind one call.

use std::{
  sync::Arc,
  time::{SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use tarpc::context;
use tokio::sync::Mutex;

use crate::{
  GroupId, NodeId, groups,
  network::transport::Libp2pNetworkFactory,
  tasks::{
    MAX_TASK_PAYLOAD_BYTES,
    handlers::TaskPayload,
    records::{TaskOpResult, TaskQueueMetrics, TaskRecord, WorkerLeaseRecord},
    rpc::{ControlNodes, TaskRpc, TaskRpcRequest, TaskRpcResponse, TaskRpcService},
    worker::{call_read, submit_command},
  },
  types_kv::TaskRequest,
};

/// How this node reaches the replicated task queue.
#[derive(Clone)]
pub enum TaskFrontend {
  /// Control node: submit directly through the local raft handle.
  Control,
  /// Worker node: go through the tarpc TaskRpc protocol to control nodes.
  Worker {
    control_nodes: Arc<Mutex<ControlNodes>>,
  },
}

/// The single task-domain API handle carried by frontends (HTTP handlers,
/// admin surfaces). Cheap to clone; construction happens once at service
/// assembly ([`crate::app`]).
#[derive(Clone)]
pub struct TaskApi {
  group_id: GroupId,
  network: Libp2pNetworkFactory,
  registry: crate::GroupRegistry,
  frontend: TaskFrontend,
}

impl TaskApi {
  pub fn new(
    network: Libp2pNetworkFactory,
    registry: crate::GroupRegistry,
    frontend: TaskFrontend,
  ) -> Self {
    Self {
      group_id: groups::TASKS.to_string(),
      network,
      registry,
      frontend,
    }
  }

  fn unix_now_secs() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs()
  }

  /// The single enqueue door: validates the payload (size cap + kind-tag
  /// decode), generates the task id, stamps the timestamps, and proposes
  /// the `TaskEnqueue` command. Every frontend enqueue MUST come through
  /// here so no path can slip an oversized or malformed payload into the
  /// raft log.
  pub async fn enqueue(
    &self,
    payload: String,
    idem_key: Option<String>,
    delay_secs: u64,
  ) -> anyhow::Result<TaskOpResult> {
    if payload.len() > MAX_TASK_PAYLOAD_BYTES {
      return Err(anyhow!(
        "task payload is {} bytes, over the {} byte limit (wasm modules must fit the raft log)",
        payload.len(),
        MAX_TASK_PAYLOAD_BYTES
      ));
    }
    // Reject unknown/malformed kinds at submit time, not at execution.
    TaskPayload::decode(&payload).map_err(|err| anyhow!(err))?;

    let now = Self::unix_now_secs();
    self
      .submit(TaskRequest::TaskEnqueue {
        id: uuid::Uuid::now_v7().to_string(),
        payload,
        run_at: now + delay_secs,
        idem_key,
        created_at: now,
      })
      .await
  }

  /// Dead-letter replay: return a permanently failed task to the queue with
  /// a fresh attempt budget. The rules live in the state machine (Failed
  /// only; committed tasks refused), so this just proposes the command.
  pub async fn replay(&self, id: String) -> anyhow::Result<TaskOpResult> {
    self
      .submit(TaskRequest::TaskReplay {
        id,
        now: Self::unix_now_secs(),
      })
      .await
  }

  /// The single write path for task state-machine commands: control nodes
  /// write through their local raft handle (following a leader hint over
  /// the network when needed), workers go through the tarpc TaskRpc.
  pub async fn submit(&self, cmd: TaskRequest) -> anyhow::Result<TaskOpResult> {
    match &self.frontend {
      TaskFrontend::Control => {
        let reply = TaskRpcService::new(self.registry.clone())
          .submit(
            context::current(),
            self.group_id.clone(),
            cmd.clone().into(),
          )
          .await;
        if reply.ok {
          let value = reply
            .value
            .ok_or_else(|| anyhow!("task command accepted but returned no result"))?;
          return sonic_rs::from_str(&value).map_err(|err| anyhow!("decode task op result: {err}"));
        }
        // Not the leader for this group: follow the hint over the network.
        if let Some(leader_id) = reply.leader_id.as_deref() {
          let leader = NodeId::new(leader_id);
          if let Some(addr) = reply.leader_addr.as_deref() {
            // Best-effort address-book refresh; submit_command below retries
            // through existing routes if this fails.
            let _ = self.network.register_node(leader.clone(), addr).await;
          }
          let control_nodes = Mutex::new(ControlNodes::new(vec![leader]));
          return submit_command(&self.network, &control_nodes, &self.group_id, cmd).await;
        }
        Err(anyhow!(
          "task command failed: {}",
          reply.error.unwrap_or_default()
        ))
      }
      TaskFrontend::Worker { control_nodes } => {
        submit_command(&self.network, control_nodes, &self.group_id, cmd).await
      }
    }
  }

  /// All task records (admin view), sorted by id.
  pub async fn list_tasks(&self) -> anyhow::Result<Vec<TaskRecord>> {
    let reply = match &self.frontend {
      TaskFrontend::Control => {
        TaskRpcService::new(self.registry.clone())
          .list_tasks(context::current(), self.group_id.clone())
          .await
      }
      TaskFrontend::Worker { control_nodes } => {
        let response = call_read(&self.network, control_nodes, || TaskRpcRequest::ListTasks {
          group_id: self.group_id.clone(),
        })
        .await?;
        match response {
          TaskRpcResponse::ListTasks(reply) => reply,
          other => return Err(anyhow!("unexpected task rpc response: {other:?}")),
        }
      }
    };
    if !reply.ok {
      return Err(anyhow!(reply.error.unwrap_or_default()));
    }
    Ok(reply.tasks)
  }

  /// All worker lease records, sorted by node id.
  pub async fn list_workers(&self) -> anyhow::Result<Vec<WorkerLeaseRecord>> {
    let reply = match &self.frontend {
      TaskFrontend::Control => {
        TaskRpcService::new(self.registry.clone())
          .list_workers(context::current(), self.group_id.clone())
          .await
      }
      TaskFrontend::Worker { control_nodes } => {
        let response = call_read(&self.network, control_nodes, || {
          TaskRpcRequest::ListWorkers {
            group_id: self.group_id.clone(),
          }
        })
        .await?;
        match response {
          TaskRpcResponse::ListWorkers(reply) => reply,
          other => return Err(anyhow!("unexpected task rpc response: {other:?}")),
        }
      }
    };
    if !reply.ok {
      return Err(anyhow!(reply.error.unwrap_or_default()));
    }
    Ok(reply.workers)
  }

  /// Point-in-time queue health snapshot.
  pub async fn metrics(&self) -> anyhow::Result<TaskQueueMetrics> {
    let reply = match &self.frontend {
      TaskFrontend::Control => {
        TaskRpcService::new(self.registry.clone())
          .metrics(context::current(), self.group_id.clone())
          .await
      }
      TaskFrontend::Worker { control_nodes } => {
        let response = call_read(&self.network, control_nodes, || TaskRpcRequest::Metrics {
          group_id: self.group_id.clone(),
        })
        .await?;
        match response {
          TaskRpcResponse::Metrics(reply) => reply,
          other => return Err(anyhow!("unexpected task rpc response: {other:?}")),
        }
      }
    };
    if !reply.ok {
      return Err(anyhow!(reply.error.unwrap_or_default()));
    }
    reply
      .metrics
      .ok_or_else(|| anyhow!("metrics reply carried no snapshot"))
  }
}

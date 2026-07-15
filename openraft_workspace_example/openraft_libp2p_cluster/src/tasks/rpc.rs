//! Task RPC: a tarpc service carried over the libp2p request_response
//! protocol `/openraft/task/1` (same envelope pattern as
//! [`crate::sqlite_sync_rpc`], client style follows
//! `tarpc_workspace_example`).
//!
//! Workers and worker-mode HTTP frontends call control nodes here. Every
//! WRITE is a replicated state-machine command ([`crate::types_kv::Request`])
//! submitted through `Raft::client_write`; non-leaders reply with a leader
//! hint that the client follows.

use tarpc::{context, server::Serve};

use crate::{
  GroupId, NodeId, openraft_group,
  store::ensure_linearizable_read,
  tasks::{
    TASK_REC_PREFIX, TASK_WORKER_PREFIX, TaskQueueMetrics, TaskRecord, WorkerLeaseRecord,
    assigned_idx_node_prefix, compute_metrics, scheduler::current_unix_secs,
  },
  typ::ClientWriteError,
  types_kv::Request as StateCommand,
};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TaskWriteReply {
  pub ok: bool,
  /// `Response.value` from the state machine (JSON `TaskOpResult`).
  pub value: Option<String>,
  pub error: Option<String>,
  pub leader_id: Option<String>,
  pub leader_addr: Option<String>,
}

impl TaskWriteReply {
  fn error(message: impl Into<String>) -> Self {
    Self {
      ok: false,
      error: Some(message.into()),
      ..Self::default()
    }
  }
}

/// One entry of the per-worker assigned index: enough for the worker to
/// claim directly (the claim command re-validates node+epoch in apply).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssignedTask {
  pub id: String,
  pub lease_epoch: u64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TaskIdsReply {
  pub ok: bool,
  pub assigned: Vec<AssignedTask>,
  pub error: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TaskRecordsReply {
  pub ok: bool,
  pub tasks: Vec<TaskRecord>,
  pub error: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct WorkerLeasesReply {
  pub ok: bool,
  pub workers: Vec<WorkerLeaseRecord>,
  pub error: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TaskMetricsReply {
  pub ok: bool,
  pub metrics: Option<TaskQueueMetrics>,
  pub error: Option<String>,
}

#[tarpc::service]
pub trait TaskRpc {
  /// Submit a task state-machine command through raft.
  async fn submit(group_id: GroupId, cmd: StateCommand) -> TaskWriteReply;
  /// Tasks currently assigned to `node_id` with their lease epochs
  /// (narrow index scan; no record reads).
  async fn list_assigned(group_id: GroupId, node_id: String) -> TaskIdsReply;
  /// All task records (admin/HTTP view).
  async fn list_tasks(group_id: GroupId) -> TaskRecordsReply;
  /// All worker lease records.
  async fn list_workers(group_id: GroupId) -> WorkerLeasesReply;
  /// Queue health snapshot (status counts, retries, worker liveness).
  async fn metrics(group_id: GroupId) -> TaskMetricsReply;
  /// Directed assignment wake, sent by the scheduler to exactly the assigned
  /// worker node (replaces the old task-assign gossip broadcast, which made
  /// every node in the cluster receive every assignment). Delivery is
  /// best-effort: a missed wake is covered by the worker's reconciliation
  /// poll.
  async fn notify_assigned(worker_node_id: String, task_id: String, lease_epoch: u64) -> bool;
}

pub type TaskRpcRequestMessage = tarpc::ClientMessage<TaskRpcRequest>;
pub type TaskRpcResponseMessage = tarpc::Response<TaskRpcResponse>;

/// Build the one-shot tarpc envelope for a request message.
pub fn task_rpc_request(message: TaskRpcRequest) -> TaskRpcRequestMessage {
  tarpc::ClientMessage::Request(tarpc::Request {
    context: context::current(),
    id: 0,
    message,
  })
}

/// Unwrap the tarpc envelope of a response.
pub fn task_rpc_response(response: TaskRpcResponseMessage) -> anyhow::Result<TaskRpcResponse> {
  match response.message {
    Ok(message) => Ok(message),
    Err(err) => Err(anyhow::anyhow!("task rpc failed: {err}")),
  }
}

/// Server side: unwrap the envelope, serve, re-wrap. Mirrors
/// `process_sqlite_sync_rpc_request`.
pub async fn process_task_rpc_request(request: TaskRpcRequestMessage) -> TaskRpcResponseMessage {
  match request {
    tarpc::ClientMessage::Request(request) => {
      let request_id = request.id;
      let response = TaskRpcService
        .serve()
        .serve(request.context, request.message)
        .await;
      tarpc::Response {
        request_id,
        message: response,
      }
    }
    tarpc::ClientMessage::Cancel { request_id, .. } => tarpc::Response {
      request_id,
      message: Err(tarpc::ServerError::new(
        std::io::ErrorKind::Interrupted,
        "task rpc cancel messages are not processed by one-shot libp2p rpc".to_string(),
      )),
    },
    _ => tarpc::Response {
      request_id: 0,
      message: Err(tarpc::ServerError::new(
        std::io::ErrorKind::InvalidInput,
        "unsupported task rpc message".to_string(),
      )),
    },
  }
}

#[derive(Clone)]
pub struct TaskRpcService;

impl TaskRpc for TaskRpcService {
  async fn submit(
    self,
    _: context::Context,
    group_id: GroupId,
    cmd: StateCommand,
  ) -> TaskWriteReply {
    let Some(group) = openraft_group(&group_id) else {
      return TaskWriteReply::error(format!("unknown group_id={group_id}"));
    };

    // Same door check as the HTTP frontend: enqueues arriving over the
    // task RPC (worker-mode frontends, external clients) must also respect
    // the payload cap before the command reaches the raft log.
    if let StateCommand::TaskEnqueue { payload, .. } = &cmd
      && payload.len() > crate::tasks::MAX_TASK_PAYLOAD_BYTES
    {
      return TaskWriteReply::error(format!(
        "task payload is {} bytes, over the {} byte limit",
        payload.len(),
        crate::tasks::MAX_TASK_PAYLOAD_BYTES
      ));
    }

    match group.raft.client_write(cmd).await {
      Ok(resp) => TaskWriteReply {
        ok: true,
        value: resp.data.value,
        error: None,
        leader_id: None,
        leader_addr: None,
      },
      Err(err) => {
        let mut reply = TaskWriteReply::error(format!("client_write failed: {err}"));
        if let Some(ClientWriteError::ForwardToLeader(forward)) = err.api_error() {
          reply.leader_id = forward.leader_id.clone().map(|id| id.to_string());
          reply.leader_addr = forward.leader_node.clone().map(|node| node.addr);
        }
        reply
      }
    }
  }

  async fn list_assigned(
    self,
    _: context::Context,
    group_id: GroupId,
    node_id: String,
  ) -> TaskIdsReply {
    match read_entries(&group_id, assigned_idx_node_prefix(&node_id)).await {
      Ok(entries) => {
        let mut assigned = Vec::with_capacity(entries.len());
        for (key, value) in entries {
          // Index value = lease epoch at assignment time.
          let Some((_, task_id)) = crate::tasks::parse_assigned_idx_key(&key) else {
            tracing::warn!(%key, "skipping malformed assigned index key");
            continue;
          };
          let Ok(lease_epoch) = value.parse::<u64>() else {
            tracing::warn!(%key, %value, "skipping assigned index entry with non-numeric epoch");
            continue;
          };
          assigned.push(AssignedTask {
            id: task_id.to_string(),
            lease_epoch,
          });
        }
        TaskIdsReply {
          ok: true,
          assigned,
          error: None,
        }
      }
      Err(err) => TaskIdsReply {
        ok: false,
        assigned: Vec::new(),
        error: Some(err),
      },
    }
  }

  async fn list_tasks(self, _: context::Context, group_id: GroupId) -> TaskRecordsReply {
    match read_entries(&group_id, TASK_REC_PREFIX.to_string()).await {
      Ok(entries) => {
        let mut tasks = Vec::with_capacity(entries.len());
        for (key, value) in entries {
          match sonic_rs::from_str::<TaskRecord>(&value) {
            Ok(record) => tasks.push(record),
            Err(err) => tracing::warn!(%key, error = ?err, "skipping corrupt task record"),
          }
        }
        tasks.sort_by(|a, b| a.id.cmp(&b.id));
        TaskRecordsReply {
          ok: true,
          tasks,
          error: None,
        }
      }
      Err(err) => TaskRecordsReply {
        ok: false,
        tasks: Vec::new(),
        error: Some(err),
      },
    }
  }

  async fn list_workers(self, _: context::Context, group_id: GroupId) -> WorkerLeasesReply {
    match read_entries(&group_id, TASK_WORKER_PREFIX.to_string()).await {
      Ok(entries) => {
        let mut workers = Vec::with_capacity(entries.len());
        for (key, value) in entries {
          match sonic_rs::from_str::<WorkerLeaseRecord>(&value) {
            Ok(record) => workers.push(record),
            Err(err) => tracing::warn!(%key, error = ?err, "skipping corrupt worker lease"),
          }
        }
        workers.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        WorkerLeasesReply {
          ok: true,
          workers,
          error: None,
        }
      }
      Err(err) => WorkerLeasesReply {
        ok: false,
        workers: Vec::new(),
        error: Some(err),
      },
    }
  }

  async fn notify_assigned(
    self,
    _: context::Context,
    worker_node_id: String,
    task_id: String,
    lease_epoch: u64,
  ) -> bool {
    crate::tasks::worker::notify_assignment(&worker_node_id, &task_id, lease_epoch);
    true
  }

  async fn metrics(self, _: context::Context, group_id: GroupId) -> TaskMetricsReply {
    let records = match Self::read_records(&group_id).await {
      Ok(records) => records,
      Err(err) => {
        return TaskMetricsReply {
          ok: false,
          metrics: None,
          error: Some(err),
        };
      }
    };
    let leases = match Self::read_leases(&group_id).await {
      Ok(leases) => leases,
      Err(err) => {
        return TaskMetricsReply {
          ok: false,
          metrics: None,
          error: Some(err),
        };
      }
    };

    TaskMetricsReply {
      ok: true,
      metrics: Some(compute_metrics(&records, &leases, current_unix_secs())),
      error: None,
    }
  }
}

impl TaskRpcService {
  async fn read_records(group_id: &str) -> Result<Vec<TaskRecord>, String> {
    let entries = read_entries(group_id, TASK_REC_PREFIX.to_string()).await?;
    let mut records = Vec::with_capacity(entries.len());
    for (key, value) in entries {
      match sonic_rs::from_str::<TaskRecord>(&value) {
        Ok(record) => records.push(record),
        Err(err) => tracing::warn!(%key, error = ?err, "skipping corrupt task record"),
      }
    }
    Ok(records)
  }

  async fn read_leases(group_id: &str) -> Result<Vec<WorkerLeaseRecord>, String> {
    let entries = read_entries(group_id, TASK_WORKER_PREFIX.to_string()).await?;
    let mut leases = Vec::with_capacity(entries.len());
    for (key, value) in entries {
      match sonic_rs::from_str::<WorkerLeaseRecord>(&value) {
        Ok(lease) => leases.push(lease),
        Err(err) => tracing::warn!(%key, error = ?err, "skipping corrupt worker lease"),
      }
    }
    Ok(leases)
  }
}

async fn read_entries(group_id: &str, prefix: String) -> Result<Vec<(String, String)>, String> {
  let Some(group) = openraft_group(group_id) else {
    return Err(format!("unknown group_id={group_id}"));
  };
  ensure_linearizable_read(&group.raft)
    .await
    .map_err(|err| format!("linearizable read failed: {err:?}"))?;
  group
    .kv_data
    .entries_with_prefix(prefix)
    .await
    .map_err(|err| format!("read task entries failed: {err}"))
}

/// Known control nodes with a sticky leader hint, shared by workers and the
/// worker-mode HTTP frontend.
#[derive(Clone, Default)]
pub struct ControlNodes {
  known: Vec<NodeId>,
  leader: Option<NodeId>,
}

impl ControlNodes {
  pub fn new(known: Vec<NodeId>) -> Self {
    Self {
      known,
      leader: None,
    }
  }

  pub fn targets(&self) -> Vec<NodeId> {
    let mut targets = self.known.clone();
    if let Some(leader) = &self.leader
      && let Some(pos) = targets.iter().position(|id| id == leader)
    {
      targets.swap(0, pos);
    }
    targets
  }

  pub fn report_leader(&mut self, leader: NodeId) {
    if !self.known.contains(&leader) {
      self.known.push(leader.clone());
    }
    self.leader = Some(leader);
  }
}

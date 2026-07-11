//! Worker-side task execution loop (the layer that octopii does NOT provide
//! and that must never move into `apply()`): claim → execute side effects on
//! exactly one node → ack, plus worker lease renewal.
//!
//! Wake-up is event driven: the scheduler publishes a `TaskAssignedMessage`
//! on the task-assign gossip topic and the swarm forwards it into the wake
//! channel below; a slow poll remains as fallback.

use std::{
  sync::{Arc, OnceLock},
  time::Duration,
};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, broadcast};

use crate::{
  GroupId, NodeId,
  network::transport::Libp2pNetworkFactory,
  signal::ShutdownRx,
  tasks::{
    MAX_TASK_ATTEMPTS, TaskOpResult, TaskRecord,
    rpc::{
      ControlNodes, TaskRpcRequest, TaskRpcResponse, TaskWriteReply, task_rpc_request,
      task_rpc_response,
    },
    scheduler::current_unix_secs,
  },
  types_kv::Request as StateCommand,
};

const WORKER_LEASE_INTERVAL: Duration = Duration::from_secs(10);
const WORKER_LEASE_TTL_SECS: u64 = 30;
const WORKER_POLL_FALLBACK: Duration = Duration::from_secs(5);
const RETRY_BACKOFF_BASE_SECS: u64 = 5;
const MAX_LEADER_REDIRECTS: usize = 3;

/// Wake channel fed by the swarm's gossip handler when the scheduler
/// announces an assignment for this node.
static TASK_WAKE_TX: OnceLock<broadcast::Sender<String>> = OnceLock::new();

fn wake_channel() -> &'static broadcast::Sender<String> {
  TASK_WAKE_TX.get_or_init(|| broadcast::channel(64).0)
}

/// Called from the swarm gossip handler on the task-assign topic.
pub fn notify_assignment(worker_node_id: &str) {
  if let Some(tx) = TASK_WAKE_TX.get() {
    let _ = tx.send(worker_node_id.to_string());
  }
}

/// The demo task type executed by workers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
  pub to: String,
}

async fn execute_task(record: &TaskRecord) -> Result<(), String> {
  let email: Email =
    sonic_rs::from_str(&record.payload).map_err(|err| format!("decode email payload: {err}"))?;
  tracing::info!(task_id = %record.id, to = %email.to, "sending email");
  Ok(())
}

/// Submit a task state-machine command to the control plane, following
/// leader hints (tarpc TaskRpc over libp2p).
pub async fn submit_command(
  network: &Libp2pNetworkFactory,
  control_nodes: &Mutex<ControlNodes>,
  group_id: &str,
  cmd: StateCommand,
) -> anyhow::Result<TaskOpResult> {
  let targets = control_nodes.lock().await.targets();
  let mut last_error: Option<String> = None;

  for base_target in targets {
    let mut target = base_target;
    for _redirect in 0 ..= MAX_LEADER_REDIRECTS {
      let request = task_rpc_request(TaskRpcRequest::Submit {
        group_id: group_id.to_string(),
        cmd: cmd.clone(),
      });
      let reply = match network.request_task_rpc(target.clone(), request).await {
        Ok(response) => match task_rpc_response(response) {
          Ok(TaskRpcResponse::Submit(reply)) => reply,
          Ok(other) => {
            last_error = Some(format!("unexpected task rpc response: {other:?}"));
            break;
          }
          Err(err) => {
            last_error = Some(err.to_string());
            break;
          }
        },
        Err(err) => {
          last_error = Some(format!("{err}"));
          break;
        }
      };

      if reply.ok {
        let value = reply
          .value
          .ok_or_else(|| anyhow!("task command accepted but returned no result"))?;
        let result: TaskOpResult =
          sonic_rs::from_str(&value).map_err(|err| anyhow!("decode task op result: {err}"))?;
        return Ok(result);
      }

      if let Some(next) = follow_leader_hint(network, control_nodes, &target, &reply).await {
        target = next;
        continue;
      }

      last_error = reply.error;
      break;
    }
  }

  Err(anyhow!(
    "task command failed on all control nodes: {}",
    last_error.unwrap_or_else(|| "no reachable control node".to_string())
  ))
}

async fn follow_leader_hint(
  network: &Libp2pNetworkFactory,
  control_nodes: &Mutex<ControlNodes>,
  current_target: &NodeId,
  reply: &TaskWriteReply,
) -> Option<NodeId> {
  let leader_id = NodeId::new(reply.leader_id.as_deref()?);
  if &leader_id == current_target {
    return None;
  }
  if let Some(addr) = reply.leader_addr.as_deref() {
    let _ = network.register_node(leader_id.clone(), addr).await;
  }
  control_nodes.lock().await.report_leader(leader_id.clone());
  Some(leader_id)
}

/// Read-style RPC against any reachable control node. Takes a builder
/// because the tarpc-generated request enum is not `Clone`.
pub async fn call_read(
  network: &Libp2pNetworkFactory,
  control_nodes: &Mutex<ControlNodes>,
  build_request: impl Fn() -> TaskRpcRequest,
) -> anyhow::Result<TaskRpcResponse> {
  let targets = control_nodes.lock().await.targets();
  let mut last_error: Option<String> = None;

  for target in targets {
    match network
      .request_task_rpc(target, task_rpc_request(build_request()))
      .await
    {
      Ok(response) => match task_rpc_response(response) {
        Ok(message) => return Ok(message),
        Err(err) => last_error = Some(err.to_string()),
      },
      Err(err) => last_error = Some(format!("{err}")),
    }
  }

  Err(anyhow!(
    "task rpc read failed on all control nodes: {}",
    last_error.unwrap_or_else(|| "no reachable control node".to_string())
  ))
}

/// Run the task worker: lease renewal + claim/execute/ack loop.
pub async fn run_task_worker(
  node_id: NodeId,
  worker_name: String,
  group_id: GroupId,
  network: Libp2pNetworkFactory,
  control_nodes: Vec<NodeId>,
  mut shutdown_rx: ShutdownRx,
) -> anyhow::Result<()> {
  let control_nodes = Arc::new(Mutex::new(ControlNodes::new(control_nodes)));
  let mut wake_rx = wake_channel().subscribe();

  // Lease renewal keeps this node in the scheduler's active-worker set.
  let lease_handle = tokio::spawn(run_lease_renewal(
    node_id.clone(),
    worker_name,
    group_id.clone(),
    network.clone(),
    control_nodes.clone(),
    shutdown_rx.clone(),
  ));

  let mut fallback = tokio::time::interval(WORKER_POLL_FALLBACK);
  fallback.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!(node_id = %node_id, "stopping task worker");
        break;
      }
      wake = wake_rx.recv() => {
        match wake {
          Ok(target) if target == node_id.to_string() => {}
          Ok(_) => continue,           // assignment for another worker
          Err(broadcast::error::RecvError::Lagged(_)) => {}
          Err(broadcast::error::RecvError::Closed) => continue,
        }
      }
      _ = fallback.tick() => {}
    }

    if let Err(err) = drain_assigned_tasks(&node_id, &group_id, &network, &control_nodes).await {
      tracing::debug!(node_id = %node_id, error = ?err, "task worker drain failed; retrying");
    }
  }

  lease_handle.abort();
  Ok(())
}

async fn drain_assigned_tasks(
  node_id: &NodeId,
  group_id: &str,
  network: &Libp2pNetworkFactory,
  control_nodes: &Mutex<ControlNodes>,
) -> anyhow::Result<()> {
  let response = call_read(network, control_nodes, || TaskRpcRequest::ListAssigned {
    group_id: group_id.to_string(),
    node_id: node_id.to_string(),
  })
  .await?;
  let TaskRpcResponse::ListAssigned(reply) = response else {
    return Err(anyhow!("unexpected list_assigned response"));
  };
  if !reply.ok {
    return Err(anyhow!(
      "list_assigned failed: {}",
      reply.error.unwrap_or_default()
    ));
  }

  for task_id in reply.ids {
    if let Err(err) = claim_and_execute(node_id, group_id, network, control_nodes, &task_id).await {
      tracing::warn!(
        node_id = %node_id,
        task_id = %task_id,
        error = ?err,
        "task execution round failed"
      );
    }
  }
  Ok(())
}

async fn claim_and_execute(
  node_id: &NodeId,
  group_id: &str,
  network: &Libp2pNetworkFactory,
  control_nodes: &Mutex<ControlNodes>,
  task_id: &str,
) -> anyhow::Result<()> {
  // Read our current assignment lease epoch from the record via claim: the
  // claim command itself validates (node, lease) atomically in apply(), so a
  // stale assignment simply yields ok=false.
  let assigned = fetch_record(network, control_nodes, group_id, task_id).await?;
  let Some(lease_epoch) = assigned.lease_epoch else {
    return Ok(());
  };

  let claim = submit_command(
    network,
    control_nodes,
    group_id,
    StateCommand::TaskClaim {
      id: task_id.to_string(),
      node_id: node_id.to_string(),
      lease_epoch,
    },
  )
  .await?;
  if !claim.ok {
    tracing::debug!(
      task_id = %task_id,
      reason = ?claim.reason,
      "task claim rejected; skipping"
    );
    return Ok(());
  }
  let record = claim
    .record
    .ok_or_else(|| anyhow!("claim succeeded but returned no record"))?;

  // Execute the side effect LOCALLY — never inside the state machine.
  let outcome = execute_task(&record).await;

  let ack = match outcome {
    Ok(()) => StateCommand::TaskDone {
      id: record.id.clone(),
      node_id: node_id.to_string(),
      lease_epoch,
      attempts: record.attempts,
    },
    Err(error) => {
      let retry_at = if record.attempts >= MAX_TASK_ATTEMPTS {
        0 // permanent failure
      } else {
        current_unix_secs() + RETRY_BACKOFF_BASE_SECS * (1 << record.attempts.min(6)) as u64
      };
      tracing::warn!(
        task_id = %record.id,
        attempts = record.attempts,
        retry_at,
        error = %error,
        "task execution failed"
      );
      StateCommand::TaskFail {
        id: record.id.clone(),
        node_id: node_id.to_string(),
        lease_epoch,
        attempts: record.attempts,
        error,
        retry_at,
      }
    }
  };

  let acked = submit_command(network, control_nodes, group_id, ack).await?;
  if !acked.ok {
    tracing::warn!(task_id = %record.id, reason = ?acked.reason, "task ack rejected as stale");
  }
  Ok(())
}

async fn fetch_record(
  network: &Libp2pNetworkFactory,
  control_nodes: &Mutex<ControlNodes>,
  group_id: &str,
  task_id: &str,
) -> anyhow::Result<TaskRecord> {
  let response = call_read(network, control_nodes, || TaskRpcRequest::ListTasks {
    group_id: group_id.to_string(),
  })
  .await?;
  let TaskRpcResponse::ListTasks(reply) = response else {
    return Err(anyhow!("unexpected list_tasks response"));
  };
  reply
    .tasks
    .into_iter()
    .find(|task| task.id == task_id)
    .ok_or_else(|| anyhow!("task {task_id} not found"))
}

async fn run_lease_renewal(
  node_id: NodeId,
  worker_name: String,
  group_id: GroupId,
  network: Libp2pNetworkFactory,
  control_nodes: Arc<Mutex<ControlNodes>>,
  mut shutdown_rx: ShutdownRx,
) {
  let mut lease_epoch = current_unix_secs();
  let mut tick = tokio::time::interval(WORKER_LEASE_INTERVAL);

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => return,
      _ = tick.tick() => {}
    }

    lease_epoch = lease_epoch.saturating_add(1);
    let lease = StateCommand::WorkerLease {
      node_id: node_id.to_string(),
      worker_name: worker_name.clone(),
      lease_epoch,
      expires_at: current_unix_secs() + WORKER_LEASE_TTL_SECS,
    };
    if let Err(err) = submit_command(&network, &control_nodes, &group_id, lease).await {
      tracing::warn!(
        node_id = %node_id,
        error = ?err,
        "worker lease renewal failed; scheduler will drop this worker if it keeps failing"
      );
    }
  }
}

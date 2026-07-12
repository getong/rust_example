//! Worker-side task execution loop (the layer that octopii does NOT provide
//! and that must never move into `apply()`): claim → execute side effects on
//! exactly one node → ack, plus worker lease renewal.
//!
//! Wake-up is event driven: the scheduler publishes a `TaskAssignedMessage`
//! (task id + lease epoch) on the task-assign gossip topic and the swarm
//! forwards it into the wake channel below, so the worker claims directly
//! with zero read RPCs. A slow reconciliation poll remains as fallback for
//! missed gossip. Executions run concurrently up to
//! [`WORKER_MAX_CONCURRENT_TASKS`], so one slow task never blocks the rest.

use std::{
  collections::HashSet,
  sync::{Arc, OnceLock},
  time::Duration,
};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Semaphore, broadcast};

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
/// Hard cap on a single task execution; a hung handler counts as a failure
/// and goes through the normal retry/backoff path.
const TASK_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);
const WORKER_LEASE_TTL_SECS: u64 = 30;
/// Reconciliation poll for missed gossip only; assignments normally arrive
/// through the wake channel with everything needed to claim.
const WORKER_POLL_FALLBACK: Duration = Duration::from_secs(30);
const RETRY_BACKOFF_BASE_SECS: u64 = 5;
const MAX_LEADER_REDIRECTS: usize = 3;
/// Per-node cap on concurrently executing tasks.
const WORKER_MAX_CONCURRENT_TASKS: usize = 4;

/// A concrete assignment forwarded from the scheduler's gossip message:
/// carries everything needed to claim, so no read RPC is required.
#[derive(Debug, Clone)]
pub struct TaskAssignment {
  pub worker_node_id: String,
  pub task_id: String,
  pub lease_epoch: u64,
}

/// Wake channel fed by the swarm's gossip handler when the scheduler
/// announces an assignment for this node.
static TASK_WAKE_TX: OnceLock<broadcast::Sender<TaskAssignment>> = OnceLock::new();

fn wake_channel() -> &'static broadcast::Sender<TaskAssignment> {
  TASK_WAKE_TX.get_or_init(|| broadcast::channel(64).0)
}

/// Called from the swarm gossip handler on the task-assign topic.
pub fn notify_assignment(worker_node_id: &str, task_id: &str, lease_epoch: u64) {
  if let Some(tx) = TASK_WAKE_TX.get() {
    let _ = tx.send(TaskAssignment {
      worker_node_id: worker_node_id.to_string(),
      task_id: task_id.to_string(),
      lease_epoch,
    });
  }
}

/// The demo task type executed by workers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
  pub to: String,
}

/// Executes the task and returns the handler result (opaque JSON) that is
/// stored on the record via `TaskDone`.
async fn execute_task(record: &TaskRecord) -> Result<Option<String>, String> {
  let email: Email =
    sonic_rs::from_str(&record.payload).map_err(|err| format!("decode email payload: {err}"))?;

  // Testable failure semantics for drills: recipients starting with "fail"
  // simulate a handler error (exercises retry/backoff and permanent failure),
  // "slow" simulates a hang (exercises the execution timeout).
  if email.to.starts_with("fail") {
    return Err(format!("simulated failure sending to {}", email.to));
  }
  if email.to.starts_with("slow") {
    tokio::time::sleep(TASK_EXECUTION_TIMEOUT + Duration::from_secs(15)).await;
  }

  tracing::info!(task_id = %record.id, to = %email.to, "sending email");
  let result = sonic_rs::to_string(&sonic_rs::json!({
    "delivered_to": email.to,
    "attempt": record.attempts,
  }))
  .map_err(|err| format!("encode task result: {err}"))?;
  Ok(Some(result))
}

/// Execute with the hard timeout applied.
async fn execute_task_with_timeout(record: &TaskRecord) -> Result<Option<String>, String> {
  match tokio::time::timeout(TASK_EXECUTION_TIMEOUT, execute_task(record)).await {
    Ok(result) => result,
    Err(_) => Err(format!(
      "task execution timed out after {}s",
      TASK_EXECUTION_TIMEOUT.as_secs()
    )),
  }
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

/// Shared context for a running worker; cloned into each spawned execution.
struct WorkerCtx {
  node_id: NodeId,
  group_id: GroupId,
  network: Libp2pNetworkFactory,
  control_nodes: Arc<Mutex<ControlNodes>>,
  /// Bounds concurrent executions.
  permits: Arc<Semaphore>,
  /// Task ids currently claimed-or-waiting on this node; dedupes the wake
  /// path against the fallback reconciliation poll.
  in_flight: Arc<Mutex<HashSet<String>>>,
}

/// Run the task worker: lease renewal + concurrent claim/execute/ack.
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

  let ctx = Arc::new(WorkerCtx {
    node_id: node_id.clone(),
    group_id,
    network,
    control_nodes,
    permits: Arc::new(Semaphore::new(WORKER_MAX_CONCURRENT_TASKS)),
    in_flight: Arc::new(Mutex::new(HashSet::new())),
  });

  let mut fallback = tokio::time::interval(WORKER_POLL_FALLBACK);
  fallback.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        // In-flight executions are abandoned; unacked tasks come back via
        // the scheduler's stuck/lease requeue path.
        tracing::info!(node_id = %node_id, "stopping task worker");
        break;
      }
      wake = wake_rx.recv() => {
        match wake {
          Ok(assignment) if assignment.worker_node_id == node_id.to_string() => {
            // The wake carries the full assignment: claim directly,
            // no list/read round-trip.
            spawn_execution(&ctx, assignment.task_id, assignment.lease_epoch).await;
            continue;
          }
          Ok(_) => continue,           // assignment for another worker
          Err(broadcast::error::RecvError::Lagged(_)) => {
            // Missed wakes: reconcile below via list_assigned.
          }
          Err(broadcast::error::RecvError::Closed) => continue,
        }
      }
      _ = fallback.tick() => {}
    }

    if let Err(err) = drain_assigned_tasks(&ctx).await {
      tracing::debug!(node_id = %node_id, error = ?err, "task worker drain failed; retrying");
    }
  }

  lease_handle.abort();
  Ok(())
}

/// Spawn one bounded, deduplicated execution. Returns immediately; the
/// spawned future waits for a concurrency permit, claims, executes, acks.
async fn spawn_execution(ctx: &Arc<WorkerCtx>, task_id: String, lease_epoch: u64) {
  if !ctx.in_flight.lock().await.insert(task_id.clone()) {
    return; // already claimed-or-waiting on this node
  }
  let ctx = ctx.clone();
  tokio::spawn(async move {
    let _permit = match ctx.permits.clone().acquire_owned().await {
      Ok(permit) => permit,
      Err(_) => {
        ctx.in_flight.lock().await.remove(&task_id);
        return;
      }
    };
    if let Err(err) = claim_and_execute(&ctx, &task_id, lease_epoch).await {
      tracing::warn!(
        node_id = %ctx.node_id,
        task_id = %task_id,
        error = ?err,
        "task execution round failed"
      );
    }
    ctx.in_flight.lock().await.remove(&task_id);
  });
}

/// Fallback reconciliation: list this node's assigned index (ids + lease
/// epochs) and spawn anything not already in flight.
async fn drain_assigned_tasks(ctx: &Arc<WorkerCtx>) -> anyhow::Result<()> {
  let response = call_read(&ctx.network, &ctx.control_nodes, || {
    TaskRpcRequest::ListAssigned {
      group_id: ctx.group_id.to_string(),
      node_id: ctx.node_id.to_string(),
    }
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

  for task in reply.assigned {
    spawn_execution(ctx, task.id, task.lease_epoch).await;
  }
  Ok(())
}

async fn claim_and_execute(ctx: &WorkerCtx, task_id: &str, lease_epoch: u64) -> anyhow::Result<()> {
  let node_id = &ctx.node_id;
  let group_id = ctx.group_id.as_str();
  let network = &ctx.network;
  let control_nodes = ctx.control_nodes.as_ref();

  // The claim command validates (node, lease_epoch) atomically in apply();
  // a stale assignment simply yields ok=false.
  let claim = submit_command(
    network,
    control_nodes,
    group_id,
    StateCommand::TaskClaim {
      id: task_id.to_string(),
      node_id: node_id.to_string(),
      lease_epoch,
      now: current_unix_secs(),
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

  // Execute the side effect LOCALLY — never inside the state machine —
  // bounded by the execution timeout.
  let outcome = execute_task_with_timeout(&record).await;

  let ack = match outcome {
    Ok(result) => StateCommand::TaskDone {
      id: record.id.clone(),
      node_id: node_id.to_string(),
      lease_epoch,
      attempts: record.attempts,
      now: current_unix_secs(),
      result,
    },
    Err(error) => {
      let now = current_unix_secs();
      let retry_at = if record.attempts >= MAX_TASK_ATTEMPTS {
        0 // permanent failure
      } else {
        now + RETRY_BACKOFF_BASE_SECS * (1 << record.attempts.min(6)) as u64
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
        now,
      }
    }
  };

  let acked = submit_command(network, control_nodes, group_id, ack).await?;
  if !acked.ok {
    tracing::warn!(task_id = %record.id, reason = ?acked.reason, "task ack rejected as stale");
  }
  Ok(())
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

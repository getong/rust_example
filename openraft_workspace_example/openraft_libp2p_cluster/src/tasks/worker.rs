//! Worker-side task execution loop (the layer that octopii does NOT provide
//! and that must never move into `apply()`): claim → execute side effects on
//! exactly one node → ack, plus worker lease renewal.
//!
//! Wake-up is event driven: the scheduler sends a directed
//! `TaskRpc::notify_assigned` RPC (task id + lease epoch) to exactly the
//! assigned worker, which feeds the wake channel below, so the worker claims
//! directly with zero read RPCs. A slow reconciliation poll remains as
//! fallback for missed wakes. Executions run concurrently up to
//! [`WORKER_MAX_CONCURRENT_TASKS`], so one slow task never blocks the rest.
//!
//! Execution itself is task-agnostic: the loop decodes the payload into the
//! [`crate::tasks::handlers::TaskPayload`] enum and calls its `execute` dispatch, so adding a task
//! type never touches this pipeline (append an enum variant instead).

use std::{
  collections::HashSet,
  sync::{Arc, OnceLock},
  time::Duration,
};

use anyhow::anyhow;
use retry::delay::Exponential;
use tokio::sync::{Mutex, Semaphore, broadcast};

/// Kept as a re-export so existing callers (HTTP frontend) keep compiling;
/// the type now lives with its handler.
pub use crate::tasks::handlers::Email;
use crate::{
  GroupId, NodeId,
  network::transport::Libp2pNetworkFactory,
  signal::ShutdownRx,
  tasks::{
    MAX_TASK_ATTEMPTS, TaskOpResult, TaskRecord,
    handlers::{TaskCtx, execute_payload},
    rpc::{
      ControlNodes, TaskRpcRequest, TaskRpcResponse, TaskWriteReply, task_rpc_request,
      task_rpc_response,
    },
    scheduler::current_unix_secs,
  },
  types_kv::TaskRequest as StateCommand,
};

/// Hard cap on a single task execution; a hung handler counts as a failure
/// and goes through the normal retry/backoff path.
pub(crate) const TASK_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);
/// Reconciliation poll for missed wakes only; assignments normally arrive
/// through the wake channel with everything needed to claim.
const WORKER_POLL_FALLBACK: Duration = Duration::from_secs(30);
const RETRY_BACKOFF_BASE_SECS: u64 = 5;

/// Retry delay: exponential backoff (base * 2^attempts, exponent capped at
/// 6) plus jitter in [0, backoff/2] so a batch of tasks that failed together
/// (downstream outage) spreads over the retry window instead of hammering
/// the recovering dependency in lockstep. The backoff ladder comes from the
/// `retry` crate's [`Exponential`] delay strategy (factor 2.0); its blocking
/// `retry::retry` loop is NOT used because retries here are scheduled
/// through raft (`retry_at`), not spun in-process. The jitter is HASHED from
/// (task id, attempt), not sampled (so no `retry::delay::jitter`): the same
/// retry decision always proposes the same `retry_at`, which keeps raft
/// re-proposals idempotent.
fn retry_backoff_secs(task_id: &str, attempts: u32) -> u64 {
  let backoff = Exponential::from_millis(RETRY_BACKOFF_BASE_SECS * 1000)
    .nth(attempts.min(6) as usize)
    .expect("Exponential delay iterator is infinite")
    .as_secs();
  let jitter =
    xxhash_rust::xxh3::xxh3_64(format!("{task_id}:{attempts}").as_bytes()) % (backoff / 2 + 1);
  backoff + jitter
}
const MAX_LEADER_REDIRECTS: usize = 3;
/// Per-node cap on concurrently executing tasks.
const WORKER_MAX_CONCURRENT_TASKS: usize = 4;

/// A concrete assignment forwarded from the scheduler's directed wake RPC:
/// carries everything needed to claim, so no read RPC is required.
#[derive(Debug, Clone)]
pub struct TaskAssignment {
  pub worker_node_id: String,
  pub task_id: String,
  pub lease_epoch: u64,
}

/// Wake channel fed by the task RPC service when the scheduler sends a
/// directed `notify_assigned` for this node.
static TASK_WAKE_TX: OnceLock<broadcast::Sender<TaskAssignment>> = OnceLock::new();

/// Default capacity of the assignment wake channel. All workers on this node
/// share one broadcast channel and each receiver filters by
/// `worker_node_id`, so the capacity must absorb assignment bursts for every
/// local worker together; an undersized channel drops wakes (`Lagged`),
/// which degrades to the slower reconciliation poll instead of losing tasks.
const TASK_WAKE_CHANNEL_CAPACITY_DEFAULT: usize = 512;

/// Env override for the wake channel capacity, read once at first use:
/// `TASK_WAKE_CHANNEL_CAPACITY=1024`.
const TASK_WAKE_CHANNEL_CAPACITY_ENV: &str = "TASK_WAKE_CHANNEL_CAPACITY";

fn wake_channel_capacity() -> usize {
  std::env::var(TASK_WAKE_CHANNEL_CAPACITY_ENV)
    .ok()
    .and_then(|raw| raw.parse::<usize>().ok())
    .filter(|capacity| *capacity > 0)
    .unwrap_or(TASK_WAKE_CHANNEL_CAPACITY_DEFAULT)
}

fn wake_channel() -> &'static broadcast::Sender<TaskAssignment> {
  TASK_WAKE_TX.get_or_init(|| broadcast::channel(wake_channel_capacity()).0)
}

/// Called from `TaskRpc::notify_assigned` (and directly by a leader waking
/// its own co-located worker).
pub fn notify_assignment(worker_node_id: &str, task_id: &str, lease_epoch: u64) {
  if let Some(tx) = TASK_WAKE_TX.get() {
    let _ = tx.send(TaskAssignment {
      worker_node_id: worker_node_id.to_string(),
      task_id: task_id.to_string(),
      lease_epoch,
    });
  }
}

/// Execute one claimed task through the [`crate::tasks::handlers::TaskPayload`] enum dispatch,
/// bounded by the hard timeout.
async fn execute_task_with_timeout(
  ctx: &WorkerCtx,
  record: &TaskRecord,
) -> Result<Option<String>, String> {
  let task_ctx = TaskCtx::with_cluster(&ctx.network, &ctx.control_nodes);
  match tokio::time::timeout(TASK_EXECUTION_TIMEOUT, execute_payload(&task_ctx, record)).await {
    Ok(result) => result,
    Err(_) => Err(format!(
      "task execution timed out after {}s",
      TASK_EXECUTION_TIMEOUT.as_secs()
    )),
  }
}

/// Submit a state-machine command to the control plane, following leader
/// hints (tarpc TaskRpc over libp2p), and return the raw write reply of the
/// node that accepted it. Works for any [`StateCommand`], not just task
/// commands — handlers use this for e.g. KV writes.
#[must_use = "the reply carries whether the raft write was accepted; dropping it hides rejected \
              writes"]
pub async fn submit_reply(
  network: &Libp2pNetworkFactory,
  control_nodes: &Mutex<ControlNodes>,
  group_id: &str,
  cmd: impl Into<crate::types_kv::Request>,
) -> anyhow::Result<TaskWriteReply> {
  let cmd: crate::types_kv::Request = cmd.into();
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
        return Ok(reply);
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

/// Submit a TASK command and decode the state machine's `TaskOpResult`.
#[must_use = "the result carries whether the state-machine command was applied; dropping it hides \
              rejected writes"]
pub async fn submit_command(
  network: &Libp2pNetworkFactory,
  control_nodes: &Mutex<ControlNodes>,
  group_id: &str,
  cmd: impl Into<crate::types_kv::Request>,
) -> anyhow::Result<TaskOpResult> {
  let reply = submit_reply(network, control_nodes, group_id, cmd).await?;
  let value = reply
    .value
    .ok_or_else(|| anyhow!("task command accepted but returned no result"))?;
  sonic_rs::from_str(&value).map_err(|err| anyhow!("decode task op result: {err}"))
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
    // Best-effort address-book refresh: the retry against the hinted leader
    // below works from existing state even if registration fails.
    let _ = network.register_node(leader_id.clone(), addr).await;
  }
  control_nodes.lock().await.report_leader(leader_id.clone());
  Some(leader_id)
}

/// Read-style RPC against any reachable control node. Takes a builder
/// because the tarpc-generated request enum is not `Clone`.
#[must_use = "the result carries the read reply or the RPC failure; dropping it hides errors"]
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
/// Executes every [`crate::tasks::handlers::TaskPayload`] kind.
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
  // dispatched by payload kind, bounded by the execution timeout.
  let outcome = execute_task_with_timeout(ctx, &record).await;

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
        now + retry_backoff_secs(&record.id, record.attempts)
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

  loop {
    // Interval and TTL are hot-reloadable (POST /config), so both are read
    // per round instead of being captured in a fixed `interval` timer.
    let config = crate::runtime_config::current();
    tokio::select! {
      _ = shutdown_rx.changed() => return,
      _ = tokio::time::sleep(config.worker_lease_interval()) => {}
    }

    lease_epoch = lease_epoch.saturating_add(1);
    let lease = StateCommand::WorkerLease {
      node_id: node_id.to_string(),
      worker_name: worker_name.clone(),
      lease_epoch,
      expires_at: current_unix_secs() + config.worker_lease_ttl_secs,
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn retry_backoff_is_bounded_and_deterministic() {
    for attempts in 0 .. 10 {
      let backoff = RETRY_BACKOFF_BASE_SECS * (1u64 << attempts.min(6));
      let delay = retry_backoff_secs("t1", attempts);
      assert!(
        (backoff ..= backoff + backoff / 2).contains(&delay),
        "attempt {attempts}: delay {delay} outside [{backoff}, {}]",
        backoff + backoff / 2
      );
      // Same (task, attempt) → same delay: raft re-proposals stay idempotent.
      assert_eq!(delay, retry_backoff_secs("t1", attempts));
    }
  }

  #[test]
  fn retry_backoff_jitter_spreads_a_failed_batch() {
    let delays: std::collections::BTreeSet<u64> = (0 .. 100)
      .map(|task| retry_backoff_secs(&format!("task-{task}"), 2))
      .collect();
    assert!(
      delays.len() > 5,
      "100 tasks failing the same attempt should spread over the jitter window, got {} distinct \
       delays",
      delays.len()
    );
  }
}

//! Leader-side task scheduler for the task group.
//!
//! Runs only on the group's current raft leader (managed by the leader
//! controller). Scheduling is event driven: the state machine bumps
//! [`crate::tasks::events`] after applying schedule-relevant commands, so a
//! pass runs immediately after an enqueue/requeue/retry instead of waiting
//! for a poll. A slow fallback timer (and a precise sleep until the next
//! future `run_at`) covers time-based work: due delayed tasks, dead-worker
//! requeue, and stuck-task detection.
//!
//! Each pass does two narrow index scans:
//!   - `task:idx:assigned:` → requeue tasks whose worker lease expired, or whose worker stayed
//!     disconnected past the suspect grace window ([`WORKER_DISCONNECT_GRACE_SECS`]);
//!   - `task:idx:queued:`   → assign due tasks to active workers.
//! Every transition is a replicated `TaskAssign`/`TaskRequeue` command; this
//! module never mutates task state directly.

use std::{
  collections::{BTreeSet, HashMap, hash_map::DefaultHasher},
  hash::{Hash, Hasher},
  str::FromStr,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use tokio::time::Instant;

use crate::{
  GroupId, NodeId,
  network::transport::Libp2pNetworkFactory,
  openraft_group,
  signal::ShutdownRx,
  tasks::{
    TASK_ASSIGNED_IDX_PREFIX, TASK_QUEUED_IDX_PREFIX, TASK_TERMINAL_IDX_PREFIX, TASK_WORKER_PREFIX,
    TaskOpResult, TaskRecord, TaskStatus, WorkerLeaseRecord, parse_assigned_idx_key,
    parse_queued_idx_key, parse_terminal_idx_key, rec_key,
    rpc::{TaskRpcRequest, task_rpc_request},
  },
  types_kv::TaskRequest as StateCommand,
};

/// A task stuck in Assigned/Running on a LIVE worker for longer than this is
/// returned to the queue (crash-free hangs: worker bug, lost wake, endless
/// execution). Dead workers are handled separately via lease liveness.
pub const TASK_STUCK_REQUEUE_SECS: u64 = 60;

/// Slow fallback between passes when no apply event arrives. Time-based work
/// (lease expiry, stuck detection) tolerates this latency; new work is
/// event-driven and does not wait for it.
pub const SCHEDULER_FALLBACK_SECS: u64 = 5;

/// How long a worker with a FRESH lease may stay disconnected (from this
/// leader's libp2p view) before its in-flight tasks are requeued.
///
/// Lease renewal is a raft write that any control node forwards to the
/// leader, so a fresh lease proves cluster-level liveness; the leader's
/// direct connection view is a single-observer signal that can go dark on an
/// asymmetric partition while the worker keeps executing. During the grace
/// window such a worker is a "suspect": it keeps its in-flight tasks but
/// receives no new assignments. Lease expiry remains the authoritative
/// kill signal and is never delayed by this grace.
pub const WORKER_DISCONNECT_GRACE_SECS: u64 = 15;

/// How often the leader looks for expired terminal records to vacuum.
const VACUUM_INTERVAL_SECS: u64 = 60;
/// Terminal (done/failed) records older than this are deleted, together
/// with their idempotency keys. Overridable via env `TASK_RETENTION_SECS`.
const DEFAULT_TASK_RETENTION_SECS: u64 = 3600;
/// Ids per TaskVacuum command; bounds raft entry size.
const VACUUM_BATCH_LIMIT: usize = 256;

fn task_retention_secs() -> u64 {
  static RETENTION: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
  *RETENTION.get_or_init(|| {
    std::env::var("TASK_RETENTION_SECS")
      .ok()
      .and_then(|raw| raw.parse().ok())
      .unwrap_or(DEFAULT_TASK_RETENTION_SECS)
  })
}

pub async fn run_task_scheduler(
  group_id: GroupId,
  network: Libp2pNetworkFactory,
  tick_interval: Duration,
  mut shutdown_rx: ShutdownRx,
) -> anyhow::Result<()> {
  // The controller's interval is treated as a lower bound; the scheduler
  // paces itself off apply events + run_at deadlines, not a tight poll.
  let fallback = tick_interval.max(Duration::from_secs(SCHEDULER_FALLBACK_SECS));
  let mut events = crate::tasks::events::subscribe();
  // Mark the current version as seen; anything applied after this point
  // (including during a pass) makes the next `changed()` fire immediately.
  events.mark_unchanged();

  let mut next_vacuum_at = 0u64;
  // Per-worker "disconnected since" tracking for the suspect grace window;
  // scoped to this leadership term (a new leader starts with a clean map,
  // so a suspect gets a full grace window under the new observer).
  let mut down_since: HashMap<String, Instant> = HashMap::new();

  loop {
    let pass_started = Instant::now();
    let next_due = match scheduler_tick(&group_id, &network, &mut down_since).await {
      Ok(next_due) => next_due,
      Err(err) => {
        tracing::warn!(group = %group_id, error = ?err, "task scheduler pass failed");
        None
      }
    };
    metrics::histogram!("task_scheduler_pass_duration_seconds", "group" => group_id.clone())
      .record(pass_started.elapsed().as_secs_f64());

    // Retention cleanup on its own slow cadence, independent of how often
    // apply events wake the loop.
    let now = current_unix_secs();
    if now >= next_vacuum_at {
      next_vacuum_at = now + VACUUM_INTERVAL_SECS;
      if let Err(err) = vacuum_expired_tasks(&group_id, now).await {
        tracing::warn!(group = %group_id, error = ?err, "task vacuum pass failed");
      }
    }

    // Sleep until the earliest future run_at if it lands before the
    // fallback; otherwise use the fallback cadence.
    let sleep_for = next_due
      .map(|due| Duration::from_secs(due.saturating_sub(current_unix_secs())))
      .map_or(fallback, |until_due| until_due.min(fallback));

    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!(group = %group_id, "stopping task scheduler");
        return Ok(());
      }
      _ = events.changed() => {
        tracing::debug!(group = %group_id, "scheduler woken by apply event");
      }
      _ = tokio::time::sleep(sleep_for) => {}
    }
  }
}

/// One scheduling pass. Returns the earliest future `run_at` seen in the
/// queued index (if any) so the caller can sleep precisely until it is due.
async fn scheduler_tick(
  group_id: &str,
  network: &Libp2pNetworkFactory,
  down_since: &mut HashMap<String, Instant>,
) -> anyhow::Result<Option<u64>> {
  let Some(group) = openraft_group(group_id) else {
    return Err(anyhow!("unknown group_id={group_id}"));
  };
  let now = current_unix_secs();

  let view = worker_view(group_id, network, now, down_since).await?;
  let workers = view.assignable;

  // 1) Requeue in-flight tasks that are unrecoverable on their worker:
  //    - the worker's lease expired, or it stayed disconnected past the suspect grace window (see
  //      WORKER_DISCONNECT_GRACE_SECS), or
  //    - the worker is alive but the task has been stuck in Assigned/Running past
  //      TASK_STUCK_REQUEUE_SECS (execution hang, lost wake-up, worker bug).
  let assigned = group
    .kv_data
    .entries_with_prefix(TASK_ASSIGNED_IDX_PREFIX.to_string())
    .await?;
  for (key, _) in assigned {
    let Some((node_id, task_id)) = parse_assigned_idx_key(&key) else {
      tracing::warn!(%key, "skipping malformed assigned index key");
      continue;
    };

    let reason = if !view.retained.contains(node_id) {
      "inactive worker"
    } else {
      match group.kv_data.get(&rec_key(task_id)).await? {
        Some(raw) => match sonic_rs::from_str::<TaskRecord>(&raw) {
          Ok(record)
            if matches!(record.status, TaskStatus::Assigned | TaskStatus::Running)
              && now.saturating_sub(record.updated_at) > TASK_STUCK_REQUEUE_SECS =>
          {
            "stuck past timeout on live worker"
          }
          Ok(_) => continue,
          Err(err) => {
            tracing::warn!(task_id = %task_id, error = ?err, "corrupt task record; skipping");
            continue;
          }
        },
        None => continue,
      }
    };

    let requeue = StateCommand::TaskRequeue {
      id: task_id.to_string(),
    };
    match group.raft.client_write(requeue.into()).await {
      Ok(resp) => {
        tracing::warn!(
          group = %group_id,
          task_id = %task_id,
          worker_node_id = %node_id,
          reason,
          result = ?resp.data.value,
          "requeued in-flight task"
        );
      }
      Err(err) => return Err(anyhow!("requeue {task_id} failed: {err}")),
    }
  }

  // 2) Assign due queued tasks (the index is sorted by run_at).
  let queued = group
    .kv_data
    .entries_with_prefix(TASK_QUEUED_IDX_PREFIX.to_string())
    .await?;
  if workers.is_empty() {
    // Nothing to assign to; retry on the fallback cadence.
    return Ok(None);
  }
  for (key, _) in queued {
    let Some((run_at, task_id)) = parse_queued_idx_key(&key) else {
      tracing::warn!(%key, "skipping malformed queued index key");
      continue;
    };
    if run_at > now {
      // Sorted index: everything after this is due later; report the
      // earliest so the caller sleeps exactly until it comes due.
      return Ok(Some(run_at));
    }

    let Some(worker) = select_worker(&workers, task_id) else {
      return Ok(None);
    };
    let assign = StateCommand::TaskAssign {
      id: task_id.to_string(),
      node_id: worker.node_id.clone(),
      lease_epoch: worker.lease_epoch,
      now,
    };
    match group.raft.client_write(assign.into()).await {
      Ok(resp) => {
        let ok = resp
          .data
          .value
          .as_deref()
          .and_then(|value| sonic_rs::from_str::<TaskOpResult>(value).ok())
          .map(|result| result.ok)
          .unwrap_or(false);
        if !ok {
          // Lost a race (the task changed state within this tick); skip.
          continue;
        }
        tracing::info!(
          group = %group_id,
          task_id = %task_id,
          worker_node_id = %worker.node_id,
          lease_epoch = worker.lease_epoch,
          "assigned task to worker"
        );
        notify_worker(network, task_id, &worker.node_id, worker.lease_epoch).await;
      }
      Err(err) => return Err(anyhow!("assign {task_id} failed: {err}")),
    }
  }

  Ok(None)
}

/// Scan the terminal index for records past retention and propose one
/// bounded `TaskVacuum`. The scan runs OUTSIDE apply; apply re-validates
/// each id, so a stale scan can never delete live tasks.
async fn vacuum_expired_tasks(group_id: &str, now: u64) -> anyhow::Result<()> {
  let Some(group) = openraft_group(group_id) else {
    return Err(anyhow!("unknown group_id={group_id}"));
  };
  let cutoff = now.saturating_sub(task_retention_secs());

  let terminal = group
    .kv_data
    .entries_with_prefix(TASK_TERMINAL_IDX_PREFIX.to_string())
    .await?;
  let mut ids = Vec::new();
  for (key, _) in terminal {
    let Some((completed_at, task_id)) = parse_terminal_idx_key(&key) else {
      tracing::warn!(%key, "skipping malformed terminal index key");
      continue;
    };
    if completed_at > cutoff {
      // Sorted by completion time: the rest is still within retention.
      break;
    }
    ids.push(task_id.to_string());
    if ids.len() >= VACUUM_BATCH_LIMIT {
      break;
    }
  }
  if ids.is_empty() {
    return Ok(());
  }

  let count = ids.len();
  match group
    .raft
    .client_write(StateCommand::TaskVacuum { ids }.into())
    .await
  {
    Ok(resp) => {
      tracing::info!(
        group = %group_id,
        candidates = count,
        result = ?resp.data.value,
        "vacuumed expired terminal tasks"
      );
      Ok(())
    }
    Err(err) => Err(anyhow!("vacuum failed: {err}")),
  }
}

/// The scheduler's view of the worker pool for one pass.
struct WorkerView {
  /// Lease-fresh AND connected workers: valid targets for new assignments.
  assignable: Vec<WorkerLeaseRecord>,
  /// Workers whose in-flight tasks must NOT be requeued: everything in
  /// `assignable` plus suspects (lease fresh but disconnected for less than
  /// the grace window). Two-phase failover: suspects keep their work but
  /// receive nothing new until they either reconnect or expire.
  retained: BTreeSet<String>,
}

async fn worker_view(
  group_id: &str,
  network: &Libp2pNetworkFactory,
  now: u64,
  down_since: &mut HashMap<String, Instant>,
) -> anyhow::Result<WorkerView> {
  let Some(group) = openraft_group(group_id) else {
    return Err(anyhow!("unknown group_id={group_id}"));
  };
  let entries = group
    .kv_data
    .entries_with_prefix(TASK_WORKER_PREFIX.to_string())
    .await?;

  let grace = Duration::from_secs(WORKER_DISCONNECT_GRACE_SECS);
  let tick_now = Instant::now();
  let mut assignable = Vec::new();
  let mut retained = BTreeSet::new();
  let mut leased_ids: BTreeSet<String> = BTreeSet::new();

  for (key, value) in entries {
    let lease: WorkerLeaseRecord = match sonic_rs::from_str(&value) {
      Ok(lease) => lease,
      Err(err) => {
        tracing::warn!(%key, error = ?err, "skipping corrupt worker lease");
        continue;
      }
    };
    leased_ids.insert(lease.node_id.clone());

    // Lease expiry is the authoritative failure signal: renewal is a raft
    // write, so a worker that cannot reach ANY control node loses its lease
    // after the TTL no matter what this leader's connection view says.
    if lease.expires_at < now {
      down_since.remove(&lease.node_id);
      continue;
    }

    // Alive = connected OR announcing recently. With on-demand connections a
    // healthy idle worker is intentionally not connected to this leader, so
    // raw connectedness alone would starve the assignable pool.
    let alive = match libp2p::PeerId::from_str(&lease.node_id) {
      Ok(peer) => network.is_peer_alive(&peer).await,
      Err(_) => false,
    };
    if alive {
      down_since.remove(&lease.node_id);
      retained.insert(lease.node_id.clone());
      assignable.push(lease);
      continue;
    }

    // Fresh lease but this leader sees neither a connection nor recent
    // announcements: a single-observer signal (asymmetric partition,
    // transient drop). Keep the worker's in-flight tasks for the grace
    // window instead of requeueing on the first blip.
    let since = *down_since.entry(lease.node_id.clone()).or_insert(tick_now);
    if tick_now.duration_since(since) < grace {
      retained.insert(lease.node_id.clone());
    } else {
      tracing::warn!(
        group = %group_id,
        worker_node_id = %lease.node_id,
        downtime = ?tick_now.duration_since(since),
        "worker lease is fresh but the worker stayed disconnected past the grace window; \
         releasing its in-flight tasks"
      );
    }
  }

  // Drop tracking for workers that no longer hold a lease at all.
  down_since.retain(|id, _| leased_ids.contains(id));

  assignable.sort_by(|a, b| a.node_id.cmp(&b.node_id));
  Ok(WorkerView {
    assignable,
    retained,
  })
}

fn select_worker<'a>(
  workers: &'a [WorkerLeaseRecord],
  task_id: &str,
) -> Option<&'a WorkerLeaseRecord> {
  if workers.is_empty() {
    return None;
  }
  let mut hasher = DefaultHasher::new();
  task_id.hash(&mut hasher);
  workers.get((hasher.finish() as usize) % workers.len())
}

/// Directed assignment wake: tell exactly the assigned worker about its new
/// task instead of gossiping the assignment to the whole cluster (the old
/// broadcast made every node receive every assignment — O(N x task rate)
/// gossip traffic). Best-effort: on failure the worker's reconciliation poll
/// picks the assignment up within its fallback interval.
async fn notify_worker(
  network: &Libp2pNetworkFactory,
  task_id: &str,
  worker_node_id: &str,
  lease_epoch: u64,
) {
  let started = Instant::now();
  // The leader may assign to its own co-located worker; RPC to self is
  // blocked at the transport, so wake the local channel directly.
  if worker_node_id == network.local_peer_id().to_string() {
    crate::tasks::worker::notify_assignment(worker_node_id, task_id, lease_epoch);
    metrics::histogram!("task_scheduler_dispatch_latency_seconds", "path" => "local")
      .record(started.elapsed().as_secs_f64());
    return;
  }

  let request = task_rpc_request(TaskRpcRequest::NotifyAssigned {
    worker_node_id: worker_node_id.to_string(),
    task_id: task_id.to_string(),
    lease_epoch,
  });
  if let Err(err) = network
    .request_task_rpc(NodeId::new(worker_node_id), request)
    .await
  {
    tracing::debug!(
      task_id = %task_id,
      worker_node_id = %worker_node_id,
      error = %err,
      "directed assignment wake failed; worker will reconcile via its fallback poll"
    );
  }
  metrics::histogram!("task_scheduler_dispatch_latency_seconds", "path" => "rpc")
    .record(started.elapsed().as_secs_f64());
}

pub fn current_unix_secs() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs()
}

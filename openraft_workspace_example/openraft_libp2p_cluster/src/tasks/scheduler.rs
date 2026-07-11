//! Leader-side task scheduler for the task group.
//!
//! Runs only on the group's current raft leader (managed by the leader
//! controller). Each tick does two narrow index scans:
//!   - `task:idx:assigned:` → requeue tasks whose worker lease is gone;
//!   - `task:idx:queued:`   → assign due tasks to active workers.
//! Every transition is a replicated `TaskAssign`/`TaskRequeue` command; this
//! module never mutates task state directly.

use std::{
  collections::{BTreeSet, hash_map::DefaultHasher},
  hash::{Hash, Hasher},
  str::FromStr,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;

use crate::{
  GroupId,
  network::{swarm::KvClient, transport::Libp2pNetworkFactory},
  openraft_group,
  signal::ShutdownRx,
  tasks::{
    TASK_ASSIGNED_IDX_PREFIX, TASK_QUEUED_IDX_PREFIX, TASK_WORKER_PREFIX, TaskOpResult,
    WorkerLeaseRecord, parse_assigned_idx_key, parse_queued_idx_key,
  },
  types_kv::Request as StateCommand,
};

/// Gossip topic used to wake a worker immediately after an assignment.
pub const TASK_ASSIGN_TOPIC: &str = "openraft/task-assign/1";

pub async fn run_task_scheduler(
  group_id: GroupId,
  network: Libp2pNetworkFactory,
  kv_client: KvClient,
  tick_interval: Duration,
  mut shutdown_rx: ShutdownRx,
) -> anyhow::Result<()> {
  let mut tick = tokio::time::interval(tick_interval);
  tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!(group = %group_id, "stopping task scheduler");
        return Ok(());
      }
      _ = tick.tick() => {
        if let Err(err) = scheduler_tick(&group_id, &network, &kv_client).await {
          tracing::warn!(group = %group_id, error = ?err, "task scheduler tick failed");
        }
      }
    }
  }
}

async fn scheduler_tick(
  group_id: &str,
  network: &Libp2pNetworkFactory,
  kv_client: &KvClient,
) -> anyhow::Result<()> {
  let Some(group) = openraft_group(group_id) else {
    return Err(anyhow!("unknown group_id={group_id}"));
  };
  let now = current_unix_secs();

  let workers = active_workers(group_id, network, now).await?;
  let active_ids: BTreeSet<&str> = workers.iter().map(|w| w.node_id.as_str()).collect();

  // 1) Requeue tasks assigned to inactive workers (narrow index scan).
  let assigned = group
    .kv_data
    .entries_with_prefix(TASK_ASSIGNED_IDX_PREFIX.to_string())
    .await?;
  for (key, _) in assigned {
    let Some((node_id, task_id)) = parse_assigned_idx_key(&key) else {
      tracing::warn!(%key, "skipping malformed assigned index key");
      continue;
    };
    if active_ids.contains(node_id) {
      continue;
    }

    let requeue = StateCommand::TaskRequeue {
      id: task_id.to_string(),
    };
    match group.raft.client_write(requeue).await {
      Ok(resp) => {
        tracing::warn!(
          group = %group_id,
          task_id = %task_id,
          worker_node_id = %node_id,
          result = ?resp.data.value,
          "requeued task assigned to inactive worker"
        );
      }
      Err(err) => return Err(anyhow!("requeue {task_id} failed: {err}")),
    }
  }

  // 2) Assign due queued tasks (the index is sorted by run_at).
  if workers.is_empty() {
    return Ok(());
  }
  let queued = group
    .kv_data
    .entries_with_prefix(TASK_QUEUED_IDX_PREFIX.to_string())
    .await?;
  for (key, _) in queued {
    let Some((run_at, task_id)) = parse_queued_idx_key(&key) else {
      tracing::warn!(%key, "skipping malformed queued index key");
      continue;
    };
    if run_at > now {
      // Sorted index: everything after this is due later.
      break;
    }

    let Some(worker) = select_worker(&workers, task_id) else {
      return Ok(());
    };
    let assign = StateCommand::TaskAssign {
      id: task_id.to_string(),
      node_id: worker.node_id.clone(),
      lease_epoch: worker.lease_epoch,
    };
    match group.raft.client_write(assign).await {
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
        notify_worker(kv_client, task_id, &worker.node_id).await;
      }
      Err(err) => return Err(anyhow!("assign {task_id} failed: {err}")),
    }
  }

  Ok(())
}

async fn active_workers(
  group_id: &str,
  network: &Libp2pNetworkFactory,
  now: u64,
) -> anyhow::Result<Vec<WorkerLeaseRecord>> {
  let Some(group) = openraft_group(group_id) else {
    return Err(anyhow!("unknown group_id={group_id}"));
  };
  let entries = group
    .kv_data
    .entries_with_prefix(TASK_WORKER_PREFIX.to_string())
    .await?;

  let mut workers = Vec::new();
  for (key, value) in entries {
    let lease: WorkerLeaseRecord = match sonic_rs::from_str(&value) {
      Ok(lease) => lease,
      Err(err) => {
        tracing::warn!(%key, error = ?err, "skipping corrupt worker lease");
        continue;
      }
    };
    if lease.expires_at < now {
      continue;
    }
    if let Ok(peer) = libp2p::PeerId::from_str(&lease.node_id)
      && !network.is_peer_connected(&peer).await
    {
      continue;
    }
    workers.push(lease);
  }
  workers.sort_by(|a, b| a.node_id.cmp(&b.node_id));
  Ok(workers)
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

async fn notify_worker(kv_client: &KvClient, task_id: &str, worker_node_id: &str) {
  use prost::Message as _;
  let msg = crate::proto::raft_kv::TaskAssignedMessage {
    task_id: task_id.to_string(),
    worker_id: worker_node_id.to_string(),
  };
  kv_client
    .publish_gossipsub(TASK_ASSIGN_TOPIC, msg.encode_to_vec())
    .await;
}

pub fn current_unix_secs() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs()
}

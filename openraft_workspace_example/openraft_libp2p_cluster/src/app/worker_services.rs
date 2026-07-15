//! Worker-node service loop and the watchers that decide when a worker
//! becomes a control node: promotion watcher (already a voter everywhere),
//! control-join watcher, and the evicted-learner re-registration loop.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::anyhow;

use super::*;
use crate::{
  NodeId,
  constants::SERVICE_TASK_WORKER,
  groups, http,
  network::{
    rpc::{AddLearnerRequest, RaftRpcOp, RaftRpcRequest, RaftRpcResponse},
    transport::Libp2pNetworkFactory,
  },
  tasks::{self, rpc::ControlNodes},
};

pub(crate) fn spawn_task_worker(
  shutdown: &mut crate::signal::ShutdownHandler,
  node_id: NodeId,
  worker_name: String,
  network: Libp2pNetworkFactory,
  control_nodes: Vec<NodeId>,
) -> tokio::task::JoinHandle<()> {
  let done = shutdown.push(SERVICE_TASK_WORKER);
  let worker_shutdown = shutdown.shutdown_rx();
  tokio::spawn(async move {
    let res = tasks::worker::run_task_worker(
      node_id,
      worker_name,
      groups::TASKS.to_string(),
      network,
      control_nodes,
      worker_shutdown,
    )
    .await;
    let _ = done.send(res);
  })
}

pub(crate) enum PromotionWatchResult {
  Promote,
  Shutdown,
}

pub(crate) enum ControlJoinWatchResult {
  Joined,
  AlreadyMember,
  Full,
  Shutdown,
}

pub(crate) async fn run_control_promotion_watcher(
  runtime: ControlRuntime,
  mut shutdown_rx: crate::signal::ShutdownRx,
) -> anyhow::Result<PromotionWatchResult> {
  let mut tick = tokio::time::interval(Duration::from_secs(CONTROL_PROMOTION_POLL_INTERVAL_SECS));
  tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        return Ok(PromotionWatchResult::Shutdown);
      }
      _ = tick.tick() => {
        if remote_openraft_voters_contain_self(
          &runtime.opt.id,
          &runtime.group_ids,
          &runtime.libp2p.network,
        )
        .await
        {
          tracing::info!(
            node_id = %runtime.opt.id,
            "local node is now a voter in every OpenRaft group; promoting worker to control"
          );
          return Ok(PromotionWatchResult::Promote);
        }
      }
    }
  }
}

pub(crate) async fn run_control_join_watcher(
  runtime: ControlRuntime,
  bootstrap_nodes: Vec<NodeId>,
  advertise_addr: String,
  mut shutdown_rx: crate::signal::ShutdownRx,
) -> anyhow::Result<ControlJoinWatchResult> {
  if bootstrap_nodes.is_empty() || runtime.opt.max_control_nodes == 0 {
    return Ok(ControlJoinWatchResult::Full);
  }

  let mut tick = tokio::time::interval(Duration::from_secs(CONTROL_JOIN_POLL_INTERVAL_SECS));
  tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        return Ok(ControlJoinWatchResult::Shutdown);
      }
      _ = tick.tick() => {
        match try_join_control_cluster(&runtime, &bootstrap_nodes, &advertise_addr).await {
          Some(JoinClusterOutcome::Joined) => return Ok(ControlJoinWatchResult::Joined),
          Some(JoinClusterOutcome::AlreadyMember) => return Ok(ControlJoinWatchResult::AlreadyMember),
          Some(JoinClusterOutcome::Full) => return Ok(ControlJoinWatchResult::Full),
          None => {}
        }
      }
    }
  }
}

pub(crate) async fn run_worker_services_until_promotion(
  runtime: ControlRuntime,
  http_addr: SocketAddr,
  known_control_nodes: Vec<NodeId>,
  bootstrap_nodes: Vec<NodeId>,
  advertise_addr: String,
  evicted_from_cluster: bool,
  shutdown_rx: crate::signal::ShutdownRx,
) -> anyhow::Result<ControlJoinWatchResult> {
  let mut worker_shutdown = linked_shutdown(shutdown_rx.clone());
  let worker_shutdown_tx = worker_shutdown.shutdown_tx();
  let control_nodes_for_learner = known_control_nodes.clone();
  let advertise_addr_for_learner = advertise_addr.clone();

  let shared_control_nodes = Arc::new(tokio::sync::Mutex::new(ControlNodes::new(
    known_control_nodes.clone(),
  )));
  let http_state = build_http_state(
    &runtime.opt,
    &runtime.identity,
    &runtime.libp2p,
    runtime.registry.clone(),
    None,
    http::TaskFrontend::Worker {
      control_nodes: shared_control_nodes,
    },
  );
  let worker_name = format!("libp2p-task-worker-{}", runtime.opt.id);
  let _http_handle = spawn_http(&mut worker_shutdown, http_addr, http_state);
  let _task_worker_handle = spawn_task_worker(
    &mut worker_shutdown,
    runtime.opt.id.clone(),
    worker_name,
    runtime.libp2p.network.clone(),
    known_control_nodes,
  );

  let mut worker_done_handle =
    tokio::spawn(async move { worker_shutdown.await_any_then_shutdown().await });
  let runtime_for_promotion = runtime.clone();
  let mut promotion_handle = tokio::spawn(run_control_promotion_watcher(
    runtime_for_promotion,
    shutdown_rx.clone(),
  ));
  let mut join_handle = tokio::spawn(run_control_join_watcher(
    runtime.clone(),
    bootstrap_nodes,
    advertise_addr,
    shutdown_rx.clone(),
  ));

  tokio::select! {
    promotion = &mut promotion_handle => {
      join_handle.abort();
      let _ = worker_shutdown_tx.send(());
      let (_tx, _rx, results) = worker_done_handle
        .await
        .map_err(|err| anyhow!("worker shutdown task failed: {err}"))?;
      collect_shutdown_errors("worker", results)?;

      match promotion.map_err(|err| anyhow!("promotion watcher task failed: {err}"))?? {
        PromotionWatchResult::Promote => Ok(ControlJoinWatchResult::AlreadyMember),
        PromotionWatchResult::Shutdown => Ok(ControlJoinWatchResult::Shutdown),
      }
    }
    join = &mut join_handle => {
      match join.map_err(|err| anyhow!("control join watcher task failed: {err}"))?? {
        ControlJoinWatchResult::Joined | ControlJoinWatchResult::AlreadyMember => {
          promotion_handle.abort();
          let _ = worker_shutdown_tx.send(());
          let (_tx, _rx, results) = worker_done_handle
            .await
            .map_err(|err| anyhow!("worker shutdown task failed: {err}"))?;
          collect_shutdown_errors("worker", results)?;
          Ok(ControlJoinWatchResult::Joined)
        }
        ControlJoinWatchResult::Full => {
          tracing::info!("automatic control join stopped; node remains a worker");
          // An evicted control node whose data was wiped re-joins the
          // cluster as a learner once the voter seats are taken.
          if evicted_from_cluster {
            tokio::spawn(run_evicted_learner_registration(
              runtime.clone(),
              control_nodes_for_learner,
              advertise_addr_for_learner,
              shutdown_rx.clone(),
            ));
          }
          match promotion_handle.await.map_err(|err| anyhow!("promotion watcher task failed: {err}"))?? {
            PromotionWatchResult::Promote => {
              let _ = worker_shutdown_tx.send(());
              let (_tx, _rx, results) = worker_done_handle
                .await
                .map_err(|err| anyhow!("worker shutdown task failed: {err}"))?;
              collect_shutdown_errors("worker", results)?;
              Ok(ControlJoinWatchResult::AlreadyMember)
            }
            PromotionWatchResult::Shutdown => {
              let (_tx, _rx, results) = worker_done_handle
                .await
                .map_err(|err| anyhow!("worker shutdown task failed: {err}"))?;
              collect_shutdown_errors("worker", results)?;
              Ok(ControlJoinWatchResult::Shutdown)
            }
          }
        }
        ControlJoinWatchResult::Shutdown => {
          promotion_handle.abort();
          let (_tx, _rx, results) = worker_done_handle
            .await
            .map_err(|err| anyhow!("worker shutdown task failed: {err}"))?;
          collect_shutdown_errors("worker", results)?;
          Ok(ControlJoinWatchResult::Shutdown)
        }
      }
    }
    worker_done = &mut worker_done_handle => {
      promotion_handle.abort();
      join_handle.abort();
      let (_tx, _rx, results) = worker_done
        .map_err(|err| anyhow!("worker shutdown task failed: {err}"))?;
      collect_shutdown_errors("worker", results)?;
      Ok(ControlJoinWatchResult::Shutdown)
    }
  }
}

/// Keep trying to register this (previously evicted) node as an OpenRaft
/// learner in every group until it succeeds or shutdown is requested.
pub(crate) async fn run_evicted_learner_registration(
  runtime: ControlRuntime,
  control_nodes: Vec<NodeId>,
  advertise_addr: String,
  mut shutdown_rx: crate::signal::ShutdownRx,
) {
  let mut tick = tokio::time::interval(Duration::from_secs(EVICTED_LEARNER_REGISTER_RETRY_SECS));

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => return,
      _ = tick.tick() => {
        match register_self_as_learner(&runtime, &control_nodes, &advertise_addr).await {
          Ok(()) => {
            tracing::info!(
              node_id = %runtime.opt.id,
              "evicted node re-registered as openraft learner in all groups"
            );
            return;
          }
          Err(err) => {
            tracing::debug!(
              node_id = %runtime.opt.id,
              error = ?err,
              "evicted learner registration attempt failed; retrying"
            );
          }
        }
      }
    }
  }
}

pub(crate) async fn register_self_as_learner(
  runtime: &ControlRuntime,
  control_nodes: &[NodeId],
  advertise_addr: &str,
) -> anyhow::Result<()> {
  for group_id in &runtime.group_ids {
    register_self_as_group_learner(runtime, control_nodes, group_id, advertise_addr).await?;
  }
  Ok(())
}

pub(crate) async fn register_self_as_group_learner(
  runtime: &ControlRuntime,
  control_nodes: &[NodeId],
  group_id: &str,
  advertise_addr: &str,
) -> anyhow::Result<()> {
  let mut last_error: Option<String> = None;

  for base_target in control_nodes {
    if base_target == &runtime.opt.id {
      continue;
    }
    let mut target = base_target.clone();

    for _redirect in 0 ..= CONTROL_JOIN_MAX_LEADER_REDIRECTS {
      let response = runtime
        .libp2p
        .network
        .request(
          target.clone(),
          RaftRpcRequest {
            group_id: group_id.to_string(),
            op: RaftRpcOp::AddLearner(AddLearnerRequest {
              node_id: runtime.opt.id.clone(),
              addr: advertise_addr.to_string(),
            }),
          },
        )
        .await;

      match response {
        Ok(RaftRpcResponse::AddLearner(resp)) if resp.ok => return Ok(()),
        Ok(RaftRpcResponse::AddLearner(resp)) => {
          if let Some(leader_id) = resp.leader_id
            && leader_id != target
            && leader_id != runtime.opt.id
          {
            if let Some(leader_addr) = resp.leader_addr.as_deref() {
              let _ = runtime
                .libp2p
                .network
                .register_node(leader_id.clone(), leader_addr)
                .await;
            }
            target = leader_id;
            continue;
          }
          last_error = resp.error;
          break;
        }
        Ok(RaftRpcResponse::Error(message)) => {
          last_error = Some(message);
          break;
        }
        Ok(other) => {
          last_error = Some(format!("unexpected add_learner response: {other:?}"));
          break;
        }
        Err(err) => {
          last_error = Some(format!("{err}"));
          break;
        }
      }
    }
  }

  Err(anyhow!(
    "register as learner for group {group_id} failed: {}",
    last_error.unwrap_or_else(|| "no reachable control node".to_string())
  ))
}

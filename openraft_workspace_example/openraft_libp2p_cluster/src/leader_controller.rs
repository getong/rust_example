use std::time::Duration;

use anyhow::anyhow;
use openraft::async_runtime::WatchReceiver;
use tokio::task::{JoinHandle, JoinSet};

use crate::{
  GroupHandleMap, GroupId, Raft, apalis_raft, groups,
  membership_guard::{self, MembershipGuardConfig},
  network::transport::Libp2pNetworkFactory,
  signal::{self, ShutdownRx, ShutdownTx},
  typ::RaftMetrics,
};

#[derive(Clone)]
enum LeaderWork {
  ApalisEmail(apalis_raft::RaftApalisStorage<apalis_raft::Email>),
  MembershipGuard {
    network: Libp2pNetworkFactory,
    config: MembershipGuardConfig,
  },
}

impl LeaderWork {
  fn name(&self) -> &'static str {
    match self {
      Self::ApalisEmail(_) => "apalis-email-scheduler",
      Self::MembershipGuard { .. } => "membership-guard",
    }
  }
}

struct RunningLeaderWork {
  group_id: GroupId,
  name: &'static str,
  stop_tx: ShutdownTx,
  handle: JoinHandle<anyhow::Result<()>>,
}

impl RunningLeaderWork {
  fn start(group_id: GroupId, raft: Raft, work: LeaderWork, interval: Duration) -> Self {
    let name = work.name();
    let (stop_tx, stop_rx) = signal::channel();
    let task_group_id = group_id.clone();
    let handle = tokio::spawn(async move {
      match work {
        LeaderWork::ApalisEmail(storage) => {
          run_apalis_email_scheduler(task_group_id, storage, interval, stop_rx).await
        }
        LeaderWork::MembershipGuard { network, config } => {
          membership_guard::run_membership_guard(task_group_id, raft, network, config, stop_rx)
            .await
        }
      }
    });

    Self {
      group_id,
      name,
      stop_tx,
      handle,
    }
  }

  async fn stop(self) -> anyhow::Result<()> {
    let _ = self.stop_tx.send(());
    match self.handle.await {
      Ok(result) => result,
      Err(err) => Err(anyhow!(
        "leader work task failed: group={}, work={}, error={err}",
        self.group_id,
        self.name
      )),
    }
  }
}

pub async fn run_leader_controller(
  groups: GroupHandleMap,
  apalis_storage: apalis_raft::RaftApalisStorage<apalis_raft::Email>,
  network: Libp2pNetworkFactory,
  membership_guard_config: Option<MembershipGuardConfig>,
  tick_interval: Duration,
  mut shutdown_rx: ShutdownRx,
) -> anyhow::Result<()> {
  if groups.is_empty() {
    return Err(anyhow!(
      "leader controller requires at least one openraft group"
    ));
  }

  let mut group_tasks = JoinSet::new();
  for (group_id, group) in groups {
    let works = leader_works_for_group(
      &group_id,
      &apalis_storage,
      &network,
      membership_guard_config.as_ref(),
    );
    group_tasks.spawn(run_group_leader_controller(
      group_id,
      group.raft,
      works,
      tick_interval,
      shutdown_rx.clone(),
    ));
  }

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!("stopping openraft leader controller");
        break;
      }
      joined = group_tasks.join_next() => {
        let Some(joined) = joined else {
          return Ok(());
        };
        match joined {
          Ok(Ok(())) => {}
          Ok(Err(err)) => return Err(err),
          Err(err) => return Err(anyhow!("openraft leader controller group task failed: {err}")),
        }
      }
    }
  }

  let mut errors = Vec::new();
  while let Some(joined) = group_tasks.join_next().await {
    match joined {
      Ok(Ok(())) => {}
      Ok(Err(err)) => errors.push(err),
      Err(err) => errors.push(anyhow!(
        "openraft leader controller group task failed: {err}"
      )),
    }
  }

  match errors.len() {
    0 => Ok(()),
    1 => Err(errors.remove(0)),
    _ => {
      let mut message = String::new();
      use std::fmt::Write as _;
      let _ = writeln!(
        &mut message,
        "openraft leader controller encountered {} errors:",
        errors.len()
      );
      for err in errors {
        let _ = writeln!(&mut message, "  {err}");
      }
      Err(anyhow!(message))
    }
  }
}

fn leader_works_for_group(
  group_id: &str,
  apalis_storage: &apalis_raft::RaftApalisStorage<apalis_raft::Email>,
  network: &Libp2pNetworkFactory,
  membership_guard_config: Option<&MembershipGuardConfig>,
) -> Vec<LeaderWork> {
  let mut works = Vec::new();

  if let Some(config) = membership_guard_config {
    works.push(LeaderWork::MembershipGuard {
      network: network.clone(),
      config: config.clone(),
    });
  }

  if group_id == groups::APALIS {
    works.push(LeaderWork::ApalisEmail(apalis_storage.clone()));
  }

  works
}

async fn run_group_leader_controller(
  group_id: GroupId,
  raft: Raft,
  works: Vec<LeaderWork>,
  tick_interval: Duration,
  mut shutdown_rx: ShutdownRx,
) -> anyhow::Result<()> {
  let mut metrics_rx = raft.metrics();
  let mut running: Vec<RunningLeaderWork> = Vec::new();

  let metrics = metrics_rx.borrow_watched().clone();
  apply_group_role(
    &group_id,
    &raft,
    &metrics,
    &works,
    &mut running,
    tick_interval,
  )
  .await?;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        stop_running_works(running).await?;
        return Ok(());
      }
      changed = metrics_rx.changed() => {
        if changed.is_err() {
          stop_running_works(running).await?;
          return Err(anyhow!("openraft metrics stream closed for group={group_id}"));
        }

        let metrics = metrics_rx.borrow_watched().clone();
        apply_group_role(&group_id, &raft, &metrics, &works, &mut running, tick_interval).await?;
      }
    }
  }
}

async fn apply_group_role(
  group_id: &str,
  raft: &Raft,
  metrics: &RaftMetrics,
  works: &[LeaderWork],
  running: &mut Vec<RunningLeaderWork>,
  tick_interval: Duration,
) -> anyhow::Result<()> {
  if metrics.state.is_leader() {
    if running.is_empty() {
      for work in works {
        tracing::info!(
          group = %group_id,
          node_id = %metrics.id,
          term = metrics.current_term,
          work = work.name(),
          "starting leader work"
        );
        running.push(RunningLeaderWork::start(
          group_id.to_string(),
          raft.clone(),
          work.clone(),
          tick_interval,
        ));
      }
    }
    return Ok(());
  }

  if !running.is_empty() {
    tracing::info!(
      group = %group_id,
      node_id = %metrics.id,
      term = metrics.current_term,
      state = ?metrics.state,
      "stopping leader work after role change"
    );
    stop_running_works(std::mem::take(running)).await?;
  }

  Ok(())
}

async fn stop_running_works(running: Vec<RunningLeaderWork>) -> anyhow::Result<()> {
  let mut errors = Vec::new();
  for running_work in running {
    if let Err(err) = running_work.stop().await {
      errors.push(err);
    }
  }

  match errors.len() {
    0 => Ok(()),
    1 => Err(errors.remove(0)),
    _ => Err(anyhow!("stopping leader works failed: {errors:?}")),
  }
}

async fn run_apalis_email_scheduler(
  group_id: GroupId,
  storage: apalis_raft::RaftApalisStorage<apalis_raft::Email>,
  interval: Duration,
  mut shutdown_rx: ShutdownRx,
) -> anyhow::Result<()> {
  let mut tick = tokio::time::interval(interval);
  tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!(group = %group_id, "stopping apalis leader scheduler");
        return Ok(());
      }
      _ = tick.tick() => {
        if let Err(err) = storage.run_leader_operations().await {
          tracing::warn!(
            group = %group_id,
            error = ?err,
            "openraft leader operation failed"
          );
        }
      }
    }
  }
}

use std::time::Duration;

use anyhow::anyhow;
use openraft::async_runtime::WatchReceiver;
use tokio::task::{JoinHandle, JoinSet};

use crate::{
  GroupHandleMap, GroupId, Raft, apalis_raft, groups,
  signal::{self, ShutdownRx, ShutdownTx},
  typ::RaftMetrics,
};

#[derive(Clone)]
enum LeaderWork {
  ApalisEmail(apalis_raft::RaftApalisStorage<apalis_raft::Email>),
}

impl LeaderWork {
  fn name(&self) -> &'static str {
    match self {
      Self::ApalisEmail(_) => "apalis-email-scheduler",
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
  fn start(group_id: GroupId, work: LeaderWork, interval: Duration) -> Self {
    let name = work.name();
    let (stop_tx, stop_rx) = signal::channel();
    let task_group_id = group_id.clone();
    let handle = tokio::spawn(async move {
      match work {
        LeaderWork::ApalisEmail(storage) => {
          run_apalis_email_scheduler(task_group_id, storage, interval, stop_rx).await
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
    let work = leader_work_for_group(&group_id, &apalis_storage);
    group_tasks.spawn(run_group_leader_controller(
      group_id,
      group.raft,
      work,
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

fn leader_work_for_group(
  group_id: &str,
  apalis_storage: &apalis_raft::RaftApalisStorage<apalis_raft::Email>,
) -> Option<LeaderWork> {
  if group_id == groups::APALIS {
    return Some(LeaderWork::ApalisEmail(apalis_storage.clone()));
  }

  None
}

async fn run_group_leader_controller(
  group_id: GroupId,
  raft: Raft,
  work: Option<LeaderWork>,
  tick_interval: Duration,
  mut shutdown_rx: ShutdownRx,
) -> anyhow::Result<()> {
  let mut metrics_rx = raft.metrics();
  let mut running = None;

  let metrics = metrics_rx.borrow_watched().clone();
  apply_group_role(
    &group_id,
    &metrics,
    work.as_ref(),
    &mut running,
    tick_interval,
  )
  .await?;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        stop_running_work(running).await?;
        return Ok(());
      }
      changed = metrics_rx.changed() => {
        if changed.is_err() {
          stop_running_work(running).await?;
          return Err(anyhow!("openraft metrics stream closed for group={group_id}"));
        }

        let metrics = metrics_rx.borrow_watched().clone();
        apply_group_role(&group_id, &metrics, work.as_ref(), &mut running, tick_interval).await?;
      }
    }
  }
}

async fn apply_group_role(
  group_id: &str,
  metrics: &RaftMetrics,
  work: Option<&LeaderWork>,
  running: &mut Option<RunningLeaderWork>,
  tick_interval: Duration,
) -> anyhow::Result<()> {
  if metrics.state.is_leader() {
    if running.is_none()
      && let Some(work) = work.cloned()
    {
      let work_name = work.name();
      tracing::info!(
        group = %group_id,
        node_id = %metrics.id,
        term = metrics.current_term,
        work = work_name,
        "starting leader work"
      );
      *running = Some(RunningLeaderWork::start(
        group_id.to_string(),
        work,
        tick_interval,
      ));
    }
    return Ok(());
  }

  if let Some(running_work) = running.take() {
    tracing::info!(
      group = %group_id,
      node_id = %metrics.id,
      term = metrics.current_term,
      state = ?metrics.state,
      work = running_work.name,
      "stopping leader work after role change"
    );
    running_work.stop().await?;
  }

  Ok(())
}

async fn stop_running_work(running: Option<RunningLeaderWork>) -> anyhow::Result<()> {
  if let Some(running_work) = running {
    running_work.stop().await?;
  }
  Ok(())
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

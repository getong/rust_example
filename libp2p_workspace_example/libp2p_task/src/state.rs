use std::{collections::HashSet, sync::Arc};

use anyhow::Context;
use libp2p::PeerId;
use tokio::sync::{Mutex, mpsc};

use crate::{
  domain::{DistributedTask, TaskStage, TaskUnderstanding},
  journal::ConsensusTaskJournal,
  raft_role::OpenRaftRoleTracker,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskDispatch {
  Enqueued,
  AlreadyHandled,
  SkippedOnNonFollower,
  WaitingForOpenRaftState,
}

#[derive(Clone)]
pub(crate) struct AppState {
  pub(crate) peer_id: PeerId,
  pub(crate) journal: ConsensusTaskJournal,
  pub(crate) incoming_tasks: mpsc::Sender<DistributedTask>,
  pub(crate) worker_done: mpsc::Sender<DistributedTask>,
  pub(crate) raft_role: OpenRaftRoleTracker,
  seen_tasks: Arc<Mutex<HashSet<String>>>,
}

impl AppState {
  pub(crate) fn new(
    peer_id: PeerId,
    journal: ConsensusTaskJournal,
    incoming_tasks: mpsc::Sender<DistributedTask>,
    worker_done: mpsc::Sender<DistributedTask>,
    raft_role: OpenRaftRoleTracker,
  ) -> Self {
    Self {
      peer_id,
      journal,
      incoming_tasks,
      worker_done,
      raft_role,
      seen_tasks: Arc::new(Mutex::new(HashSet::new())),
    }
  }

  pub(crate) async fn enqueue_task_once(
    &self,
    task: DistributedTask,
    accepted_stage: TaskStage,
    accepted_detail: impl Into<String>,
  ) -> anyhow::Result<TaskDispatch> {
    let accepted_detail = accepted_detail.into();

    let openraft_state = self.raft_role.state().await;
    let Some(openraft_state) = openraft_state else {
      self
        .journal
        .append(TaskUnderstanding::new(
          &task,
          accepted_stage,
          self.peer_id,
          accepted_detail,
        ))
        .await?;
      self
        .journal
        .append(TaskUnderstanding::new(
          &task,
          TaskStage::WaitingForOpenRaftState,
          self.peer_id,
          "openraft server state is unknown; apalis execution is delayed",
        ))
        .await?;
      tracing::warn!(
        task_id = %task.id,
        openraft_node_id = self.raft_role.local_node_id(),
        "openraft server state is unknown; not enqueueing task into apalis"
      );
      return Ok(TaskDispatch::WaitingForOpenRaftState);
    };

    if !self.mark_task_seen(&task).await {
      return Ok(TaskDispatch::AlreadyHandled);
    }

    self
      .journal
      .append(TaskUnderstanding::new(
        &task,
        accepted_stage,
        self.peer_id,
        accepted_detail,
      ))
      .await?;

    if !self.raft_role.is_follower().await {
      self
        .journal
        .append(TaskUnderstanding::new(
          &task,
          TaskStage::SkippedOnOpenRaftNonFollower,
          self.peer_id,
          format!(
            "local openraft state is {openraft_state:?}; apalis task must run only on a follower",
          ),
        ))
        .await?;
      tracing::info!(
        task_id = %task.id,
        openraft_state = ?openraft_state,
        "skipping apalis task because local openraft state is not follower"
      );
      return Ok(TaskDispatch::SkippedOnNonFollower);
    }

    let leader_detail = self
      .raft_role
      .current_leader()
      .await
      .map_or_else(|| "unknown".to_string(), |leader_id| leader_id);

    self
      .journal
      .append(TaskUnderstanding::new(
        &task,
        TaskStage::OpenRaftFollowerConfirmed,
        self.peer_id,
        format!(
          "openraft leader={leader_detail}; local node={} is {openraft_state:?} and may run \
           apalis task",
          self.raft_role.local_node_id(),
        ),
      ))
      .await?;

    self
      .incoming_tasks
      .send(task.clone())
      .await
      .context("enqueue task for apalis worker")?;
    self
      .journal
      .append(TaskUnderstanding::new(
        &task,
        TaskStage::EnqueuedInApalis,
        self.peer_id,
        "task enqueued into apalis memory backend",
      ))
      .await?;

    Ok(TaskDispatch::Enqueued)
  }

  async fn mark_task_seen(&self, task: &DistributedTask) -> bool {
    let mut seen_tasks = self.seen_tasks.lock().await;
    let inserted = seen_tasks.insert(task.id.clone());

    if !inserted {
      tracing::debug!(task_id = %task.id, "task already handled on this node");
    }

    inserted
  }
}

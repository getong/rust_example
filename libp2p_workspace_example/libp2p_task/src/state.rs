use std::{collections::HashSet, sync::Arc};

use anyhow::Context;
use libp2p::PeerId;
use tokio::sync::{Mutex, mpsc};

use crate::{
  domain::{DistributedTask, TaskStage, TaskUnderstanding},
  journal::ConsensusTaskJournal,
};

#[derive(Clone)]
pub(crate) struct AppState {
  pub(crate) peer_id: PeerId,
  pub(crate) journal: ConsensusTaskJournal,
  pub(crate) incoming_tasks: mpsc::Sender<DistributedTask>,
  pub(crate) worker_done: mpsc::Sender<DistributedTask>,
  seen_tasks: Arc<Mutex<HashSet<String>>>,
}

impl AppState {
  pub(crate) fn new(
    peer_id: PeerId,
    journal: ConsensusTaskJournal,
    incoming_tasks: mpsc::Sender<DistributedTask>,
    worker_done: mpsc::Sender<DistributedTask>,
  ) -> Self {
    Self {
      peer_id,
      journal,
      incoming_tasks,
      worker_done,
      seen_tasks: Arc::new(Mutex::new(HashSet::new())),
    }
  }

  pub(crate) async fn enqueue_task_once(
    &self,
    task: DistributedTask,
    accepted_stage: TaskStage,
    accepted_detail: impl Into<String>,
  ) -> anyhow::Result<bool> {
    let inserted = {
      let mut seen_tasks = self.seen_tasks.lock().await;
      seen_tasks.insert(task.id.clone())
    };

    if !inserted {
      tracing::debug!(task_id = %task.id, "task already queued on this node");
      return Ok(false);
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

    Ok(true)
  }
}

use std::time::{SystemTime, UNIX_EPOCH};

use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DistributedTask {
  pub(crate) id: String,
  pub(crate) kind: String,
  pub(crate) payload: String,
  pub(crate) submitted_by: String,
  pub(crate) submitted_at_ms: u128,
}

impl DistributedTask {
  pub(crate) fn demo(submitted_by: PeerId) -> Self {
    Self {
      id: Uuid::new_v4().to_string(),
      kind: "email.send".to_string(),
      payload: "to=demo@example.com subject=hello-from-libp2p-apalis".to_string(),
      submitted_by: submitted_by.to_string(),
      submitted_at_ms: now_ms(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TaskUnderstanding {
  pub(crate) task_id: String,
  pub(crate) stage: TaskStage,
  pub(crate) node: String,
  pub(crate) detail: String,
  pub(crate) at_ms: u128,
}

impl TaskUnderstanding {
  pub(crate) fn new(
    task: &DistributedTask,
    stage: TaskStage,
    node: PeerId,
    detail: impl Into<String>,
  ) -> Self {
    Self {
      task_id: task.id.clone(),
      stage,
      node: node.to_string(),
      detail: detail.into(),
      at_ms: now_ms(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TaskStage {
  Published,
  SubmittedLocally,
  ReceivedFromLibp2p,
  EnqueuedInApalis,
  StartedByApalis,
  FinishedByApalis,
}

fn now_ms() -> u128 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |duration| duration.as_millis())
}

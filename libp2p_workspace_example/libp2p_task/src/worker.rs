use std::time::Duration;

use apalis::prelude::*;
use apalis_core::backend::dequeue;
use tokio::sync::mpsc;

use crate::{
  domain::{DistributedTask, TaskStage, TaskUnderstanding},
  state::AppState,
};

pub(crate) fn spawn_apalis_worker(
  mut incoming_rx: mpsc::Receiver<DistributedTask>,
  state: AppState,
) {
  let storage = dequeue::backend::<DistributedTask>(Duration::from_millis(100));
  let mut task_sink = storage.clone();

  tokio::spawn(async move {
    while let Some(task) = incoming_rx.recv().await {
      if let Err(err) = task_sink.push(task.clone()).await {
        tracing::error!(task_id = %task.id, error = %err, "failed to push task into apalis");
      } else {
        tracing::info!(task_id = %task.id, "pushed task into apalis");
      }
    }
  });

  tokio::spawn(async move {
    let worker = WorkerBuilder::new("libp2p-apalis-worker")
      .backend(storage)
      .data(state)
      .build(process_task);

    if let Err(err) = worker.run().await {
      tracing::error!(error = %err, "apalis worker stopped with error");
    }
  });
}

async fn process_task(task: DistributedTask, state: Data<AppState>) -> Result<(), BoxDynError> {
  state
    .journal
    .append(TaskUnderstanding::new(
      &task,
      TaskStage::StartedByApalis,
      state.peer_id,
      format!("processing {} with payload `{}`", task.kind, task.payload),
    ))
    .await?;

  println!(
    "apalis worker handled task={} kind={} payload={}",
    task.id, task.kind, task.payload
  );

  state
    .journal
    .append(TaskUnderstanding::new(
      &task,
      TaskStage::FinishedByApalis,
      state.peer_id,
      "task finished by apalis worker",
    ))
    .await?;

  let _ = state.worker_done.send(task).await;
  Ok(())
}

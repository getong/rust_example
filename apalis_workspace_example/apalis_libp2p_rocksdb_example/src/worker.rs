use anyhow::Result;
use apalis::prelude::*;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::{
  local_backend::LocalBackend,
  model::{TaskResponse, TaskStatus, WorkerJob},
  store::TaskStore,
};

pub async fn spawn_worker(node_name: String, store: TaskStore) -> Result<mpsc::Sender<WorkerJob>> {
  let (worker_tx, mut worker_rx) = mpsc::channel::<WorkerJob>(128);
  let (backend_tx, backend_rx) = mpsc::channel::<WorkerJob>(128);

  tokio::spawn(async move {
    while let Some(job) = worker_rx.recv().await {
      if backend_tx.send(job).await.is_err() {
        error!("local apalis backend receiver closed");
        break;
      }
    }
  });

  let backend = LocalBackend::new(backend_rx);
  let worker_name = format!("{node_name}-apalis-worker");
  let worker_store = store.clone();
  tokio::spawn(async move {
    let result = WorkerBuilder::new(worker_name)
      .backend(backend)
      .data(worker_store)
      .data(node_name)
      .build(process_job)
      .run()
      .await;

    if let Err(err) = result {
      error!("{err:#}");
    }
  });

  Ok(worker_tx)
}

async fn process_job(
  job: WorkerJob,
  store: Data<TaskStore>,
  node_name: Data<String>,
) -> Result<(), BoxDynError> {
  let task = job.task.clone();
  let node = node_name.to_string();

  store.put_status(
    &task,
    TaskStatus::Received,
    Some(node.clone()),
    Some("accepted by worker".to_string()),
  )?;
  store.put_status(
    &task,
    TaskStatus::Running,
    Some(node.clone()),
    Some("running in apalis".to_string()),
  )?;

  tokio::time::sleep(std::time::Duration::from_millis(750)).await;

  let output = format!("processed {} from {}", task.payload, node);
  store.update_with_output(
    &task,
    TaskStatus::Completed,
    Some(node.clone()),
    output.clone(),
  )?;

  let response = TaskResponse::accepted(task.id.clone(), output, node);
  if !job.reply.send(response).await {
    info!(task_id = %task.id, "scheduler dropped task reply");
  }

  Ok(())
}

use anyhow::Result;
use apalis::prelude::*;
use tracing::error;

use crate::{
  model::{DistributedTask, TaskStatus},
  rocksdb_backend::RocksdbBackend,
  store::TaskStore,
};

pub async fn spawn_worker(node_name: String, store: TaskStore) -> Result<()> {
  let backend = RocksdbBackend::new(store.clone(), node_name.clone());
  let worker_name = format!("{node_name}-apalis-worker");
  let worker_store = store.clone();
  tokio::spawn(async move {
    let result = WorkerBuilder::new(worker_name)
      .backend(backend)
      .data(worker_store)
      .data(node_name)
      .build(process_task)
      .run()
      .await;

    if let Err(err) = result {
      error!("{err:#}");
    }
  });

  Ok(())
}

async fn process_task(
  task: DistributedTask,
  store: Data<TaskStore>,
  node_name: Data<String>,
) -> Result<(), BoxDynError> {
  let node = node_name.to_string();

  tokio::time::sleep(std::time::Duration::from_millis(750)).await;

  let output = format!("processed {} from {}", task.payload, node);
  store.update_with_output(&task, TaskStatus::Completed, Some(node.clone()), output)?;

  Ok(())
}

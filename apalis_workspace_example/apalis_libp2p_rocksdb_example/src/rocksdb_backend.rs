use std::{io, time::Duration};

use apalis::prelude::{Backend, Task, TaskId};
use futures::{
  StreamExt,
  stream::{self, BoxStream},
};
use tower_layer::Identity;

use crate::{model::DistributedTask, store::TaskStore};

pub struct RocksdbBackend {
  store: TaskStore,
  node_name: String,
  poll_interval: Duration,
}

impl RocksdbBackend {
  #[must_use]
  pub fn new(store: TaskStore, node_name: String) -> Self {
    Self {
      store,
      node_name,
      poll_interval: Duration::from_millis(200),
    }
  }
}

impl Backend for RocksdbBackend {
  type Args = DistributedTask;
  type IdType = String;
  type Context = ();
  type Error = io::Error;
  type Stream = BoxStream<'static, Result<Option<Task<DistributedTask, (), String>>, Self::Error>>;
  type Beat = futures::stream::Empty<Result<(), Self::Error>>;
  type Layer = Identity;

  fn heartbeat(&self, _: &apalis::prelude::WorkerContext) -> Self::Beat {
    futures::stream::empty()
  }

  fn middleware(&self) -> Self::Layer {
    Identity::new()
  }

  fn poll(self, _: &apalis::prelude::WorkerContext) -> Self::Stream {
    stream::unfold(self, poll_rocksdb).boxed()
  }
}

async fn poll_rocksdb(
  backend: RocksdbBackend,
) -> Option<(
  Result<Option<Task<DistributedTask, (), String>>, io::Error>,
  RocksdbBackend,
)> {
  loop {
    let store = backend.store.clone();
    let node_name = backend.node_name.clone();
    let claimed = tokio::task::spawn_blocking(move || store.claim_next_received(&node_name)).await;

    match claimed {
      Ok(Ok(Some(task))) => return Some((Ok(Some(build_task(task))), backend)),
      Ok(Ok(None)) => tokio::time::sleep(backend.poll_interval).await,
      Ok(Err(err)) => {
        tokio::time::sleep(backend.poll_interval).await;
        return Some((Err(io::Error::other(err)), backend));
      }
      Err(err) => {
        tokio::time::sleep(backend.poll_interval).await;
        return Some((Err(io::Error::other(err)), backend));
      }
    }
  }
}

fn build_task(task: DistributedTask) -> Task<DistributedTask, (), String> {
  let task_id = task.id.clone();
  Task::builder(task)
    .with_task_id(TaskId::new(task_id))
    .build()
}

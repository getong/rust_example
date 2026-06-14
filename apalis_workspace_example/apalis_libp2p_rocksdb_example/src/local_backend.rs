use std::{
  pin::Pin,
  task::{Context, Poll},
};

use apalis::prelude::{Backend, Task};
use futures::Stream;
use tokio::sync::mpsc;
use tower_layer::Identity;

use crate::model::WorkerJob;

pub struct LocalBackend {
  receiver: mpsc::Receiver<WorkerJob>,
}

impl LocalBackend {
  #[must_use]
  pub fn new(receiver: mpsc::Receiver<WorkerJob>) -> Self {
    Self { receiver }
  }
}

impl Backend for LocalBackend {
  type Args = WorkerJob;
  type IdType = String;
  type Context = ();
  type Error = std::io::Error;
  type Stream = LocalTaskStream;
  type Beat = futures::stream::Empty<Result<(), Self::Error>>;
  type Layer = Identity;

  fn heartbeat(&self, _: &apalis::prelude::WorkerContext) -> Self::Beat {
    futures::stream::empty()
  }

  fn middleware(&self) -> Self::Layer {
    Identity::new()
  }

  fn poll(self, _: &apalis::prelude::WorkerContext) -> Self::Stream {
    LocalTaskStream {
      receiver: self.receiver,
    }
  }
}

pub struct LocalTaskStream {
  receiver: mpsc::Receiver<WorkerJob>,
}

impl Stream for LocalTaskStream {
  type Item = Result<Option<Task<WorkerJob, (), String>>, std::io::Error>;

  fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    match self.receiver.poll_recv(cx) {
      Poll::Ready(Some(job)) => {
        let task_id = job.task.id.clone();
        let task = Task::builder(job)
          .with_task_id(apalis::prelude::TaskId::new(task_id))
          .build();
        Poll::Ready(Some(Ok(Some(task))))
      }
      Poll::Ready(None) => Poll::Ready(None),
      Poll::Pending => Poll::Pending,
    }
  }
}

use std::{
  convert::Infallible,
  error::Error,
  fmt,
  marker::PhantomData,
  str::FromStr,
  time::{SystemTime, UNIX_EPOCH},
};

use apalis::prelude::{
  Acknowledge, AcknowledgeLayer, Backend, BackendExt, BoxDynError, Codec, Status, Task, TaskId,
  TaskSink, TaskSinkError, TaskStream, WorkerBuilder, WorkerContext,
};
use apalis_core::backend::queue::Queue;
use futures::{FutureExt, Stream, StreamExt, future::BoxFuture, stream};
use openraft::async_runtime::WatchReceiver;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::time::{Duration, sleep};

use crate::{
  Raft,
  network::Router,
  rocksstore_crud::RocksStateMachine,
  types_kv::{QueueCommand, QueueResponse, TaskRecord, TaskResult, TaskStatus},
};

const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Default)]
pub struct SonicCodec<Output> {
  _output: PhantomData<Output>,
}

impl<T> Codec<T> for SonicCodec<Vec<u8>>
where
  T: Serialize + DeserializeOwned,
{
  type Compact = Vec<u8>;
  type Error = sonic_rs::Error;

  fn encode(val: &T) -> Result<Self::Compact, Self::Error> {
    sonic_rs::to_vec(val)
  }

  fn decode(val: &Self::Compact) -> Result<T, Self::Error> {
    sonic_rs::from_slice(val)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoTask {
  pub task_id: String,
  pub payload: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RaftTaskContext {
  pub lock_by: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct RaftTaskId(String);

impl RaftTaskId {
  pub fn new(id: impl Into<String>) -> Self {
    Self(id.into())
  }
}

impl Default for RaftTaskId {
  fn default() -> Self {
    Self(unique_task_id())
  }
}

impl fmt::Display for RaftTaskId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl FromStr for RaftTaskId {
  type Err = Infallible;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(Self::new(s))
  }
}

pub struct RaftApalisStorage<Args, C = SonicCodec<Vec<u8>>> {
  queue: Queue,
  raft: Raft,
  /// Router for forwarding writes to the current leader from any node.
  router: Router,
  state_machine: RocksStateMachine,
  poll_interval: Duration,
  /// Maximum number of attempts before a failing task is permanently marked Failed.
  max_attempts: usize,
  _args: PhantomData<Args>,
  _codec: PhantomData<C>,
}

impl<Args> RaftApalisStorage<Args> {
  pub fn new(
    queue_name: impl Into<String>,
    raft: Raft,
    router: Router,
    state_machine: RocksStateMachine,
  ) -> Self {
    Self {
      queue: Queue::from(queue_name.into()),
      raft,
      router,
      state_machine,
      poll_interval: POLL_INTERVAL,
      max_attempts: 3,
      _args: PhantomData,
      _codec: PhantomData,
    }
  }
}

impl<Args, C> RaftApalisStorage<Args, C> {
  pub fn with_poll_interval(mut self, interval: Duration) -> Self {
    self.poll_interval = interval;
    self
  }

  pub fn with_max_attempts(mut self, max_attempts: usize) -> Self {
    self.max_attempts = max_attempts;
    self
  }

  pub async fn list_tasks(&self) -> Result<Vec<TaskRecordView>, RaftApalisError> {
    let mut tasks = self.state_machine.tasks().await?;
    tasks.sort_by(|a, b| a.task_id.cmp(&b.task_id));
    Ok(tasks.into_iter().map(TaskRecordView::from).collect())
  }

  async fn persist_task(
    &self,
    mut task: Task<Vec<u8>, RaftTaskContext, RaftTaskId>,
  ) -> Result<(), RaftApalisError> {
    let task_id = task
      .parts
      .task_id
      .get_or_insert_with(|| TaskId::new(RaftTaskId::default()))
      .clone();
    task.parts.status.store(Status::Queued);

    let record = TaskRecord {
      task_id: task_id.to_string(),
      payload: task.args,
      attempts: task.parts.attempt.current(),
      status: TaskStatus::Pending,
      run_at: task.parts.run_at,
      lock_by: None,
      result: None,
    };

    match self
      .raft_write(QueueCommand::Submit { task: record })
      .await?
    {
      QueueResponse::Submitted { .. } => Ok(()),
      other => Err(RaftApalisError::new(format!(
        "unexpected submit response: {other:?}"
      ))),
    }
  }

  async fn try_claim_next(
    &self,
    worker_id: &str,
  ) -> Result<Option<Task<Vec<u8>, RaftTaskContext, RaftTaskId>>, RaftApalisError> {
    let response = self
      .raft_write(QueueCommand::Claim {
        worker_id: worker_id.to_string(),
        now: current_unix_secs(),
      })
      .await?;

    match response {
      QueueResponse::Claimed(Some(record)) => record.into_compact_task().map(Some),
      QueueResponse::Claimed(None) => Ok(None),
      other => Err(RaftApalisError::new(format!(
        "unexpected claim response: {other:?}"
      ))),
    }
  }

  async fn raft_write(&self, command: QueueCommand) -> Result<QueueResponse, RaftApalisError> {
    // If this node is the current leader, write directly.  Otherwise find the
    // leader from metrics and forward — this lets any cluster node participate.
    let metrics = self.raft.metrics().borrow_watched().clone();
    if metrics.state.is_leader() {
      return self
        .raft
        .client_write(command)
        .await
        .map(|r| r.data)
        .map_err(|err| RaftApalisError::new(format!("{err:?}")));
    }

    let leader_id = metrics
      .current_leader
      .ok_or_else(|| RaftApalisError::new("no raft leader available"))?;
    let leader = self
      .router
      .get_raft(leader_id)
      .await
      .ok_or_else(|| RaftApalisError::new(format!("leader node {leader_id} not in router")))?;
    leader
      .client_write(command)
      .await
      .map(|r| r.data)
      .map_err(|err| RaftApalisError::new(format!("{err:?}")))
  }
}

impl<Args, C> Backend for RaftApalisStorage<Args, C>
where
  Args: DeserializeOwned + Send + Sync + 'static,
  C: Codec<Args, Compact = Vec<u8>> + Send + Sync + Unpin + 'static,
  C::Error: Error + Send + Sync + 'static,
{
  type Args = Args;
  type IdType = RaftTaskId;
  type Context = RaftTaskContext;
  type Error = RaftApalisError;
  type Stream = TaskStream<Task<Args, RaftTaskContext, RaftTaskId>, Self::Error>;
  type Beat = stream::BoxStream<'static, Result<(), Self::Error>>;
  type Layer = AcknowledgeLayer<RaftApalisAck<C>>;

  fn heartbeat(&self, _worker: &WorkerContext) -> Self::Beat {
    stream::unfold(self.poll_interval, |interval| async move {
      sleep(interval).await;
      Some((Ok(()), interval))
    })
    .boxed()
  }

  fn middleware(&self) -> Self::Layer {
    AcknowledgeLayer::new(RaftApalisAck {
      raft: self.raft.clone(),
      router: self.router.clone(),
      max_attempts: self.max_attempts,
      _codec: PhantomData,
    })
  }

  fn poll(self, worker: &WorkerContext) -> Self::Stream {
    self
      .poll_compact(worker)
      .map(|item| {
        item.and_then(|task| {
          task
            .map(|task| task.try_map(|payload| C::decode(&payload)))
            .transpose()
            .map_err(RaftApalisError::codec)
        })
      })
      .boxed()
  }
}

impl<Args, C> BackendExt for RaftApalisStorage<Args, C>
where
  Args: DeserializeOwned + Send + Sync + 'static,
  C: Codec<Args, Compact = Vec<u8>> + Send + Sync + Unpin + 'static,
  C::Error: Error + Send + Sync + 'static,
{
  type Codec = C;
  type Compact = Vec<u8>;
  type CompactStream = TaskStream<Task<Vec<u8>, RaftTaskContext, RaftTaskId>, Self::Error>;

  fn get_queue(&self) -> Queue {
    self.queue.clone()
  }

  fn poll_compact(self, worker: &WorkerContext) -> Self::CompactStream {
    let worker_id = worker.name().clone();
    stream::unfold(self, move |storage| {
      let worker_id = worker_id.clone();
      async move {
        sleep(storage.poll_interval).await;
        let item = match storage.try_claim_next(&worker_id).await {
          Ok(task) => Ok(task),
          Err(err) => {
            tracing::warn!(worker_id = %worker_id, error = ?err, "apalis raft poll failed");
            Ok(None)
          }
        };
        Some((item, storage))
      }
    })
    .boxed()
  }
}

impl<Args, C> TaskSink<Args> for RaftApalisStorage<Args, C>
where
  Args: DeserializeOwned + Send + Sync + 'static,
  C: Codec<Args, Compact = Vec<u8>> + Send + Sync + Unpin + 'static,
  C::Error: Error + Send + Sync + 'static,
{
  async fn push(&mut self, task: Args) -> Result<(), TaskSinkError<Self::Error>> {
    let encoded = C::encode(&task).map_err(|err| TaskSinkError::CodecError(err.into()))?;
    self.persist_task(Task::new(encoded)).await?;
    Ok(())
  }

  async fn push_bulk(&mut self, tasks: Vec<Args>) -> Result<(), TaskSinkError<Self::Error>> {
    let now = current_unix_secs();
    let records = tasks
      .iter()
      .map(|task| {
        C::encode(task)
          .map(|payload| TaskRecord::pending(unique_task_id(), payload, now))
          .map_err(|err| TaskSinkError::CodecError(err.into()))
      })
      .collect::<Result<Vec<_>, _>>()?;
    self
      .raft_write(QueueCommand::SubmitBatch { tasks: records })
      .await
      .map(|_| ())
      .map_err(TaskSinkError::PushError)
  }

  async fn push_stream(
    &mut self,
    mut tasks: impl Stream<Item = Args> + Unpin + Send,
  ) -> Result<(), TaskSinkError<Self::Error>> {
    while let Some(task) = tasks.next().await {
      self.push(task).await?;
    }
    Ok(())
  }

  async fn push_task(
    &mut self,
    task: Task<Args, Self::Context, Self::IdType>,
  ) -> Result<(), TaskSinkError<Self::Error>> {
    let task =
      task.try_map(|args| C::encode(&args).map_err(|err| TaskSinkError::CodecError(err.into())))?;
    self.persist_task(task).await?;
    Ok(())
  }

  async fn push_all(
    &mut self,
    mut tasks: impl Stream<Item = Task<Args, Self::Context, Self::IdType>> + Unpin + Send,
  ) -> Result<(), TaskSinkError<Self::Error>> {
    let mut records = Vec::new();
    while let Some(task) = tasks.next().await {
      let encoded = task
        .try_map(|args| C::encode(&args).map_err(|err| TaskSinkError::CodecError(err.into())))?;
      let task_id = encoded
        .parts
        .task_id
        .as_ref()
        .map(|id| id.to_string())
        .unwrap_or_else(unique_task_id);
      records.push(TaskRecord {
        task_id,
        payload: encoded.args,
        attempts: encoded.parts.attempt.current(),
        status: TaskStatus::Pending,
        run_at: encoded.parts.run_at,
        lock_by: None,
        result: None,
      });
    }
    if records.is_empty() {
      return Ok(());
    }
    self
      .raft_write(QueueCommand::SubmitBatch { tasks: records })
      .await
      .map(|_| ())
      .map_err(TaskSinkError::PushError)
  }
}

impl<Args, C> Clone for RaftApalisStorage<Args, C> {
  fn clone(&self) -> Self {
    Self {
      queue: self.queue.clone(),
      raft: self.raft.clone(),
      router: self.router.clone(),
      state_machine: self.state_machine.clone(),
      poll_interval: self.poll_interval,
      max_attempts: self.max_attempts,
      _args: PhantomData,
      _codec: PhantomData,
    }
  }
}

impl<Args, C> fmt::Debug for RaftApalisStorage<Args, C> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("RaftApalisStorage")
      .field("queue", &self.queue)
      .field("poll_interval", &self.poll_interval)
      .finish()
  }
}

#[derive(Clone)]
pub struct RaftApalisAck<C = SonicCodec<Vec<u8>>> {
  raft: Raft,
  router: Router,
  max_attempts: usize,
  _codec: PhantomData<C>,
}

impl<C> fmt::Debug for RaftApalisAck<C> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("RaftApalisAck").finish()
  }
}

impl<C, Res> Acknowledge<Res, RaftTaskContext, RaftTaskId> for RaftApalisAck<C>
where
  Res: Serialize + Send + Sync + 'static,
  C: Codec<Res, Compact = Vec<u8>> + Send + Sync + 'static,
  C::Error: Error + Send + Sync + 'static,
{
  type Error = RaftApalisError;
  type Future = BoxFuture<'static, Result<(), Self::Error>>;

  fn ack(
    &mut self,
    res: &Result<Res, BoxDynError>,
    parts: &apalis::prelude::Parts<RaftTaskContext, RaftTaskId>,
  ) -> Self::Future {
    let Some(task_id) = parts.task_id.clone() else {
      return futures::future::ready(Err(RaftApalisError::new("missing task id"))).boxed();
    };

    let task_id = task_id.to_string();
    let max_attempts = self.max_attempts;
    let command = match res {
      Ok(value) => match C::encode(value) {
        Ok(payload) => QueueCommand::Complete {
          task_id,
          result: payload,
        },
        Err(err) => return futures::future::ready(Err(RaftApalisError::codec(err))).boxed(),
      },
      Err(err) => QueueCommand::Fail {
        task_id,
        reason: err.to_string(),
        // Retry if the task has not yet exhausted its allowed attempts.
        retry: parts.attempt.current() < max_attempts,
      },
    };

    let raft = self.raft.clone();
    let router = self.router.clone();
    async move {
      let metrics = raft.metrics().borrow_watched().clone();
      if metrics.state.is_leader() {
        return raft
          .client_write(command)
          .await
          .map(|_| ())
          .map_err(|err| RaftApalisError::new(format!("{err:?}")));
      }

      let leader_id = metrics
        .current_leader
        .ok_or_else(|| RaftApalisError::new("no raft leader available"))?;
      let leader = router
        .get_raft(leader_id)
        .await
        .ok_or_else(|| RaftApalisError::new(format!("leader node {leader_id} not in router")))?;
      leader
        .client_write(command)
        .await
        .map(|_| ())
        .map_err(|err| RaftApalisError::new(format!("{err:?}")))
    }
    .boxed()
  }
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskRecordView {
  pub task_id: String,
  pub status: String,
  pub attempts: usize,
  pub run_at: u64,
  pub lock_by: Option<String>,
  pub result_ok: Option<bool>,
  pub error: Option<String>,
}

impl From<TaskRecord> for TaskRecordView {
  fn from(task: TaskRecord) -> Self {
    let result_ok = task.result.as_ref().map(|result| result.ok);
    let error = task.result.and_then(|result| result.error);
    Self {
      task_id: task.task_id,
      status: task.status.as_str().to_string(),
      attempts: task.attempts,
      run_at: task.run_at,
      lock_by: task.lock_by,
      result_ok,
      error,
    }
  }
}

impl TaskRecord {
  fn into_compact_task(
    self,
  ) -> Result<Task<Vec<u8>, RaftTaskContext, RaftTaskId>, RaftApalisError> {
    let task_id = TaskId::from_str(&self.task_id).map_err(|err| {
      RaftApalisError::new(format!(
        "invalid stored apalis task id `{}`: {err}",
        self.task_id
      ))
    })?;
    Ok(
      Task::builder(self.payload)
        .with_task_id(task_id)
        .with_attempt(apalis::prelude::Attempt::new_with_value(self.attempts))
        .with_status(Status::Running)
        .run_at_timestamp(self.run_at)
        .with_ctx(RaftTaskContext {
          lock_by: self.lock_by,
        })
        .build(),
    )
  }
}

#[derive(Debug)]
pub struct RaftApalisError {
  message: String,
}

impl RaftApalisError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }

  fn codec(err: impl Error) -> Self {
    Self::new(err.to_string())
  }
}

impl fmt::Display for RaftApalisError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.message)
  }
}

impl Error for RaftApalisError {}

impl From<std::io::Error> for RaftApalisError {
  fn from(value: std::io::Error) -> Self {
    Self::new(value.to_string())
  }
}

pub async fn execute_demo_task(task: DemoTask) -> Result<TaskResult, BoxDynError> {
  tracing::info!(task_id = %task.task_id, payload = %task.payload, "executing task");
  sleep(Duration::from_millis(50)).await;
  Ok(TaskResult {
    ok: true,
    payload: format!("result-of-{}", task.task_id).into_bytes(),
    error: None,
  })
}

pub async fn run_demo_worker(
  worker_name: impl AsRef<str>,
  storage: RaftApalisStorage<DemoTask>,
  stop_after: Duration,
) -> anyhow::Result<()> {
  let worker = WorkerBuilder::new(worker_name.as_ref())
    .backend(storage)
    .build(execute_demo_task);

  worker
    .run_until(async move {
      sleep(stop_after).await;
      Ok::<(), apalis::prelude::WorkerError>(())
    })
    .await
    .map_err(|err| anyhow::anyhow!("apalis worker failed: {err}"))
}

fn current_unix_secs() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs()
}

fn unique_task_id() -> String {
  uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
  use apalis::prelude::{Attempt, Task};

  use super::*;

  #[test]
  fn task_record_roundtrips_compact_task() {
    let task = Task::builder(vec![1, 2, 3])
      .with_task_id(TaskId::new(RaftTaskId::new("task-1")))
      .with_attempt(Attempt::new_with_value(2))
      .with_status(Status::Running)
      .run_at_timestamp(42)
      .with_ctx(RaftTaskContext {
        lock_by: Some("worker-1".to_string()),
      })
      .build();

    let record = TaskRecord {
      task_id: task.parts.task_id.clone().expect("task id").to_string(),
      payload: task.args,
      attempts: task.parts.attempt.current(),
      status: TaskStatus::Running,
      run_at: task.parts.run_at,
      lock_by: task.parts.ctx.lock_by,
      result: None,
    };

    let rebuilt = record.into_compact_task().expect("rebuilt task");
    assert_eq!(
      rebuilt.parts.task_id.expect("rebuilt id").to_string(),
      "task-1"
    );
    assert_eq!(rebuilt.args, vec![1, 2, 3]);
    assert_eq!(rebuilt.parts.attempt.current(), 2);
    assert_eq!(rebuilt.parts.ctx.lock_by.as_deref(), Some("worker-1"));
  }
}

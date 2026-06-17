use std::{
  convert::Infallible,
  error::Error,
  fmt,
  marker::PhantomData,
  str::FromStr,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use apalis::prelude::{
  Acknowledge, AcknowledgeLayer, Backend, BackendExt, BoxDynError, Codec, Status, Task, TaskId,
  TaskSink, TaskSinkError, TaskStream, WorkerBuilder, WorkerContext,
};
use apalis_codec::json::JsonCodec;
use apalis_core::backend::queue::Queue;
use futures::{FutureExt, Stream, StreamExt, future::BoxFuture, stream};
use openraft::async_runtime::WatchReceiver;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::time::sleep;
use types_kv::Request as KvWriteRequest;

use crate::{
  GroupHandle,
  network::{swarm::KvClient, transport::parse_p2p_addr},
  proto::raft_kv::{
    ErrorResponse, RaftKvRequest, SetValueRequest, raft_kv_request::Op as KvRequestOp,
    raft_kv_response::Op as KvResponseOp,
  },
  store::{KvData, ensure_linearizable_read},
  typ::Raft,
};

const TASK_KEY_PART: &str = "task";
const IDEMPOTENCY_KEY_PART: &str = "idem";
const POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
  pub to: String,
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

pub struct RaftApalisStorage<Args, C = JsonCodec<Vec<u8>>> {
  group_id: String,
  queue: Queue,
  raft: Raft,
  kv_data: KvData,
  kv_client: KvClient,
  poll_interval: Duration,
  _args: PhantomData<Args>,
  _codec: PhantomData<C>,
}

impl<Args> RaftApalisStorage<Args> {
  pub fn new(group_id: impl Into<String>, group: GroupHandle, kv_client: KvClient) -> Self {
    let group_id = group_id.into();
    Self {
      queue: Queue::from(group_id.clone()),
      group_id,
      raft: group.raft,
      kv_data: group.kv_data,
      kv_client,
      poll_interval: POLL_INTERVAL,
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

  async fn write_record(&self, key: String, record: StoredTask) -> Result<(), RaftApalisError> {
    let value = serde_json::to_string(&record)?;
    raft_set(
      &self.raft,
      &self.kv_client,
      self.group_id.clone(),
      key,
      value,
    )
    .await
  }

  async fn write_raw(&self, key: String, value: String) -> Result<(), RaftApalisError> {
    raft_set(
      &self.raft,
      &self.kv_client,
      self.group_id.clone(),
      key,
      value,
    )
    .await
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
      group_id: self.group_id.clone(),
      raft: self.raft.clone(),
      kv_client: self.kv_client.clone(),
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
    stream::unfold((self, worker_id), |(storage, worker_id)| async move {
      sleep(storage.poll_interval).await;
      let item = storage.try_claim_next(&worker_id).await;
      Some((item, (storage, worker_id)))
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
    for task in tasks {
      self.push(task).await?;
    }
    Ok(())
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
    while let Some(task) = tasks.next().await {
      self.push_task(task).await?;
    }
    Ok(())
  }
}

impl<Args, C> RaftApalisStorage<Args, C> {
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

    let record = StoredTask::from_compact_task(task)?;
    if let Some(idempotency_key) = record.idempotency_key.as_ref() {
      let key = idempotency_key_record_key(self.queue.as_ref(), idempotency_key);
      self.write_raw(key, task_id.to_string()).await?;
    }

    self
      .write_record(
        task_record_key(self.queue.as_ref(), &task_id.to_string()),
        record,
      )
      .await
  }

  async fn try_claim_next(
    &self,
    worker_id: &str,
  ) -> Result<Option<Task<Vec<u8>, RaftTaskContext, RaftTaskId>>, RaftApalisError> {
    let metrics = self.raft.metrics().borrow_watched().clone();
    if !metrics.state.is_leader() {
      return Ok(None);
    }

    ensure_linearizable_read(&self.raft)
      .await
      .map_err(|err| RaftApalisError::new(format!("{err:?}")))?;

    let mut entries = self.kv_data.entries().await?;
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let prefix = task_key_prefix(self.queue.as_ref());
    let now = current_unix_secs();
    for (key, value) in entries {
      if !key.starts_with(&prefix) {
        continue;
      }

      let mut record = match StoredTask::decode(&value) {
        Ok(record) => record,
        Err(err) => {
          tracing::warn!(%key, error = ?err, "skipping invalid apalis task record");
          continue;
        }
      };

      if record.status != StoredStatus::Queued || record.run_at > now {
        continue;
      }

      record.status = StoredStatus::Running;
      record.lock_by = Some(worker_id.to_string());
      let task = record.clone().into_compact_task()?;
      self.write_record(key, record).await?;
      return Ok(Some(task));
    }

    Ok(None)
  }
}

impl<Args, C> Clone for RaftApalisStorage<Args, C> {
  fn clone(&self) -> Self {
    Self {
      group_id: self.group_id.clone(),
      queue: self.queue.clone(),
      raft: self.raft.clone(),
      kv_data: self.kv_data.clone(),
      kv_client: self.kv_client.clone(),
      poll_interval: self.poll_interval,
      _args: PhantomData,
      _codec: PhantomData,
    }
  }
}

impl<Args, C> fmt::Debug for RaftApalisStorage<Args, C> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("RaftApalisStorage")
      .field("group_id", &self.group_id)
      .field("queue", &self.queue)
      .field("poll_interval", &self.poll_interval)
      .finish()
  }
}

#[derive(Clone)]
pub struct RaftApalisAck<C = JsonCodec<Vec<u8>>> {
  group_id: String,
  raft: Raft,
  kv_client: KvClient,
  _codec: PhantomData<C>,
}

impl<C> fmt::Debug for RaftApalisAck<C> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("RaftApalisAck")
      .field("group_id", &self.group_id)
      .finish()
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

    let key = task_record_key(&self.group_id, &task_id.to_string());
    let status = if res.is_ok() {
      StoredStatus::Done
    } else {
      StoredStatus::Failed
    };
    let result = match res {
      Ok(value) => C::encode(value)
        .map(|payload| TaskResultRecord {
          ok: true,
          payload,
          error: None,
        })
        .map_err(RaftApalisError::codec),
      Err(err) => Ok(TaskResultRecord {
        ok: false,
        payload: Vec::new(),
        error: Some(err.to_string()),
      }),
    };

    let raft = self.raft.clone();
    let kv_client = self.kv_client.clone();
    let group_id = self.group_id.clone();
    let lock_by = parts.ctx.lock_by.clone();
    let attempt = parts.attempt.current();
    let run_at = parts.run_at;
    let idempotency_key = parts.idempotency_key.clone();

    async move {
      let result = result?;
      let record = StoredTask {
        task_id: task_id.to_string(),
        payload: Vec::new(),
        attempts: attempt,
        status,
        run_at,
        idempotency_key,
        lock_by,
        result: Some(result),
      };
      let value = serde_json::to_string(&record)?;
      raft_set(&raft, &kv_client, group_id, key, value).await
    }
    .boxed()
  }
}

async fn raft_set(
  raft: &Raft,
  kv_client: &KvClient,
  group_id: String,
  key: String,
  value: String,
) -> Result<(), RaftApalisError> {
  let metrics = raft.metrics().borrow_watched().clone();
  if metrics.state.is_leader() {
    raft
      .client_write(KvWriteRequest::Set { key, value })
      .await
      .map_err(|err| RaftApalisError::new(format!("{err:?}")))?;
    return Ok(());
  }

  let Some(leader_id) = metrics.current_leader else {
    return Err(RaftApalisError::new("no leader available"));
  };
  let Some(node) = metrics.membership_config.membership().get_node(&leader_id) else {
    return Err(RaftApalisError::new("leader node not found in membership"));
  };
  let (peer, addr) =
    parse_p2p_addr(&node.addr).map_err(|err| RaftApalisError::new(err.to_string()))?;
  kv_client
    .connect(peer, addr)
    .await
    .map_err(|err| RaftApalisError::new(format!("connect to leader failed: {err}")))?;
  let response = kv_client
    .request(
      peer,
      RaftKvRequest {
        group_id,
        op: Some(KvRequestOp::Set(SetValueRequest { key, value })),
      },
    )
    .await
    .map_err(|err| RaftApalisError::new(format!("forward to leader failed: {err}")))?;

  match response.op {
    Some(KvResponseOp::Set(resp)) if resp.ok => Ok(()),
    Some(KvResponseOp::Error(ErrorResponse { message })) => Err(RaftApalisError::new(message)),
    other => Err(RaftApalisError::new(format!(
      "unexpected raft kv response: {other:?}"
    ))),
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredTask {
  task_id: String,
  payload: Vec<u8>,
  attempts: usize,
  status: StoredStatus,
  run_at: u64,
  idempotency_key: Option<String>,
  lock_by: Option<String>,
  result: Option<TaskResultRecord>,
}

impl StoredTask {
  fn from_compact_task(
    task: Task<Vec<u8>, RaftTaskContext, RaftTaskId>,
  ) -> Result<Self, RaftApalisError> {
    let task_id = task
      .parts
      .task_id
      .ok_or_else(|| RaftApalisError::new("missing task id"))?;
    Ok(Self {
      task_id: task_id.to_string(),
      payload: task.args,
      attempts: task.parts.attempt.current(),
      status: StoredStatus::from(task.parts.status.load()),
      run_at: task.parts.run_at,
      idempotency_key: task.parts.idempotency_key,
      lock_by: task.parts.ctx.lock_by,
      result: None,
    })
  }

  fn into_compact_task(
    self,
  ) -> Result<Task<Vec<u8>, RaftTaskContext, RaftTaskId>, RaftApalisError> {
    let task_id = TaskId::from_str(&self.task_id).map_err(|err| {
      RaftApalisError::new(format!(
        "invalid stored apalis task id `{}`: {err}",
        self.task_id
      ))
    })?;
    let mut task = Task::builder(self.payload)
      .with_task_id(task_id)
      .with_attempt(apalis::prelude::Attempt::new_with_value(self.attempts))
      .with_status(Status::from(self.status))
      .run_at_timestamp(self.run_at)
      .with_ctx(RaftTaskContext {
        lock_by: self.lock_by,
      });
    if let Some(idempotency_key) = self.idempotency_key {
      task = task.with_idempotency_key(idempotency_key);
    }
    Ok(task.build())
  }

  fn decode(value: &str) -> Result<Self, RaftApalisError> {
    serde_json::from_str(value).map_err(Into::into)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskResultRecord {
  ok: bool,
  payload: Vec<u8>,
  error: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
enum StoredStatus {
  Pending,
  Queued,
  Running,
  Done,
  Failed,
  Killed,
}

impl From<Status> for StoredStatus {
  fn from(status: Status) -> Self {
    match status {
      Status::Pending => Self::Pending,
      Status::Queued => Self::Queued,
      Status::Running => Self::Running,
      Status::Done => Self::Done,
      Status::Failed => Self::Failed,
      Status::Killed => Self::Killed,
      _ => Self::Failed,
    }
  }
}

impl From<StoredStatus> for Status {
  fn from(status: StoredStatus) -> Self {
    match status {
      StoredStatus::Pending => Self::Pending,
      StoredStatus::Queued => Self::Queued,
      StoredStatus::Running => Self::Running,
      StoredStatus::Done => Self::Done,
      StoredStatus::Failed => Self::Failed,
      StoredStatus::Killed => Self::Killed,
    }
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

impl From<anyhow::Error> for RaftApalisError {
  fn from(value: anyhow::Error) -> Self {
    Self::new(value.to_string())
  }
}

impl From<serde_json::Error> for RaftApalisError {
  fn from(value: serde_json::Error) -> Self {
    Self::new(value.to_string())
  }
}

pub async fn send_email(task: Email) -> Result<(), BoxDynError> {
  tracing::info!(to = %task.to, "sending email");
  Ok(())
}

pub fn build_email_storage(
  group_id: impl Into<String>,
  group: GroupHandle,
  kv_client: KvClient,
) -> RaftApalisStorage<Email> {
  RaftApalisStorage::new(group_id, group, kv_client)
}

pub async fn run_email_worker(
  worker_name: impl AsRef<str>,
  storage: RaftApalisStorage<Email>,
  mut shutdown_rx: crate::signal::ShutdownRx,
) -> anyhow::Result<()> {
  let worker = WorkerBuilder::new(worker_name)
    .backend(storage)
    .build(send_email);
  worker
    .run_until(async move {
      let _ = shutdown_rx.changed().await;
      Ok::<(), apalis::prelude::WorkerError>(())
    })
    .await
    .map_err(|err| anyhow::anyhow!("apalis email worker failed: {err}"))
}

pub fn task_key_prefix(queue: &str) -> String {
  format!("apalis:{queue}:{TASK_KEY_PART}:")
}

pub fn task_record_key(queue: &str, task_id: &str) -> String {
  format!("{}{task_id}", task_key_prefix(queue))
}

pub fn idempotency_key_record_key(queue: &str, key: &str) -> String {
  format!("apalis:{queue}:{IDEMPOTENCY_KEY_PART}:{key}")
}

fn current_unix_secs() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs()
}

fn unique_task_id() -> String {
  let nanos = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_nanos();
  format!("{nanos:x}")
}

#[cfg(test)]
mod tests {
  use apalis::prelude::{Attempt, Task};

  use super::*;

  #[test]
  fn stored_task_roundtrips_compact_task() {
    let task = Task::builder(vec![1, 2, 3])
      .with_task_id(TaskId::new(RaftTaskId::new("task-1")))
      .with_attempt(Attempt::new_with_value(2))
      .with_status(Status::Queued)
      .run_at_timestamp(123)
      .with_ctx(RaftTaskContext {
        lock_by: Some("worker-a".to_string()),
      })
      .with_idempotency_key("idem-1")
      .build();

    let record = StoredTask::from_compact_task(task).expect("stored task");
    let encoded = serde_json::to_string(&record).expect("encode");
    let decoded = StoredTask::decode(&encoded).expect("decode");
    let task = decoded.into_compact_task().expect("compact task");

    assert_eq!(task.args, vec![1, 2, 3]);
    assert_eq!(task.parts.task_id.expect("task id").to_string(), "task-1");
    assert_eq!(task.parts.attempt.current(), 2);
    assert_eq!(task.parts.status.load(), Status::Queued);
    assert_eq!(task.parts.run_at, 123);
    assert_eq!(task.parts.ctx.lock_by, Some("worker-a".to_string()));
    assert_eq!(task.parts.idempotency_key, Some("idem-1".to_string()));
  }

  #[test]
  fn task_keys_are_namespaced_by_queue() {
    assert_eq!(
      task_record_key("apalis", "abc"),
      "apalis:apalis:task:abc".to_string()
    );
    assert_eq!(
      idempotency_key_record_key("apalis", "email-1"),
      "apalis:apalis:idem:email-1".to_string()
    );
  }
}

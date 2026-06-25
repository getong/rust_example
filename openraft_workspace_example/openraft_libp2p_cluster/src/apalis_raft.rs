use std::{
  collections::{BTreeSet, hash_map::DefaultHasher},
  convert::Infallible,
  error::Error,
  fmt,
  hash::{Hash, Hasher},
  marker::PhantomData,
  str::FromStr,
  sync::Arc,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use apalis::prelude::{
  Acknowledge, AcknowledgeLayer, Backend, BackendExt, BoxDynError, Codec, Status, Task, TaskId,
  TaskSink, TaskSinkError, TaskStream, WorkerBuilder, WorkerContext,
};
use apalis_core::backend::queue::Queue;
use futures::{FutureExt, Stream, StreamExt, future::BoxFuture, stream};
use openraft::{ServerState, async_runtime::WatchReceiver};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::{sync::Mutex, time::sleep};

use crate::{
  GroupHandle, NodeId,
  network::{
    swarm::KvClient,
    transport::{Libp2pNetworkFactory, parse_p2p_addr},
  },
  proto::raft_kv::{
    ErrorResponse, ListPrefixRequest, RaftKvRequest, SetValueRequest,
    raft_kv_request::Op as KvRequestOp, raft_kv_response::Op as KvResponseOp,
  },
  store::{KvData, ensure_linearizable_read},
  typ::Raft,
  types_kv::Request as KvWriteRequest,
};

const TASK_KEY_PART: &str = "task";
const IDEMPOTENCY_KEY_PART: &str = "idem";
const WORKER_KEY_PART: &str = "worker";
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const WORKER_LEASE_INTERVAL: Duration = Duration::from_secs(10);
const WORKER_LEASE_TTL: Duration = Duration::from_secs(30);

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
pub struct Email {
  pub to: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RaftTaskContext {
  pub lock_by: Option<String>,
  #[serde(default)]
  pub assigned_node_id: Option<String>,
  #[serde(default)]
  pub lease_epoch: Option<u64>,
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
  node_id: NodeId,
  group_id: String,
  queue: Queue,
  backend: RaftApalisBackend,
  kv_client: KvClient,
  poll_interval: Duration,
  claimed: Arc<Mutex<BTreeSet<String>>>,
  _args: PhantomData<Args>,
  _codec: PhantomData<C>,
}

#[derive(Clone)]
enum RaftApalisBackend {
  Control {
    raft: Raft,
    kv_data: KvData,
  },
  Worker {
    network: Libp2pNetworkFactory,
    control_nodes: Arc<Vec<NodeId>>,
  },
}

impl<Args> RaftApalisStorage<Args> {
  pub fn new(
    node_id: NodeId,
    group_id: impl Into<String>,
    group: GroupHandle,
    kv_client: KvClient,
  ) -> Self {
    let group_id = group_id.into();
    Self {
      node_id,
      queue: Queue::from(group_id.clone()),
      group_id,
      backend: RaftApalisBackend::Control {
        raft: group.raft,
        kv_data: group.kv_data,
      },
      kv_client,
      poll_interval: POLL_INTERVAL,
      claimed: Arc::new(Mutex::new(BTreeSet::new())),
      _args: PhantomData,
      _codec: PhantomData,
    }
  }

  pub fn worker(
    node_id: NodeId,
    group_id: impl Into<String>,
    network: Libp2pNetworkFactory,
    control_nodes: Vec<NodeId>,
    kv_client: KvClient,
  ) -> Self {
    let group_id = group_id.into();
    Self {
      node_id,
      queue: Queue::from(group_id.clone()),
      group_id,
      backend: RaftApalisBackend::Worker {
        network,
        control_nodes: Arc::new(control_nodes),
      },
      kv_client,
      poll_interval: POLL_INTERVAL,
      claimed: Arc::new(Mutex::new(BTreeSet::new())),
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
    let value = sonic_rs::to_string(&record)?;
    self.write_raw(key, value).await
  }

  async fn write_raw(&self, key: String, value: String) -> Result<(), RaftApalisError> {
    self
      .backend
      .write_raw(&self.kv_client, self.group_id.clone(), key, value)
      .await
  }

  async fn entries_with_prefix(
    &self,
    prefix: &str,
  ) -> Result<Vec<(String, String)>, RaftApalisError> {
    self
      .backend
      .entries_with_prefix(&self.group_id, prefix)
      .await
  }

  pub async fn list_tasks(&self) -> Result<Vec<TaskRecordView>, RaftApalisError> {
    let prefix = task_key_prefix(self.queue.as_ref());
    let mut tasks = Vec::new();
    for (key, value) in self.entries_with_prefix(&prefix).await? {
      let record = match StoredTask::decode(&value) {
        Ok(record) => record,
        Err(err) => {
          tracing::warn!(%key, error = ?err, "skipping invalid apalis task record");
          continue;
        }
      };
      tasks.push(record.view());
    }
    tasks.sort_by(|a, b| a.task_id.cmp(&b.task_id));
    Ok(tasks)
  }

  pub async fn list_workers(&self) -> Result<Vec<WorkerRecord>, RaftApalisError> {
    let prefix = worker_key_prefix(self.queue.as_ref());
    let mut workers = Vec::new();
    for (key, value) in self.entries_with_prefix(&prefix).await? {
      let worker = match WorkerRecord::decode(&value) {
        Ok(worker) => worker,
        Err(err) => {
          tracing::warn!(%key, error = ?err, "skipping invalid apalis worker record");
          continue;
        }
      };
      workers.push(worker);
    }
    workers.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    Ok(workers)
  }
}

impl RaftApalisBackend {
  async fn write_raw(
    &self,
    kv_client: &KvClient,
    group_id: String,
    key: String,
    value: String,
  ) -> Result<(), RaftApalisError> {
    match self {
      Self::Control { raft, .. } => raft_set(raft, kv_client, group_id, key, value).await,
      Self::Worker {
        network,
        control_nodes,
      } => write_raw_to_control_nodes(network, control_nodes, group_id, key, value).await,
    }
  }

  async fn entries_with_prefix(
    &self,
    group_id: &str,
    prefix: &str,
  ) -> Result<Vec<(String, String)>, RaftApalisError> {
    match self {
      Self::Control { raft, kv_data } => {
        ensure_linearizable_read(raft)
          .await
          .map_err(|err| RaftApalisError::new(format!("{err:?}")))?;
        let mut entries = kv_data.entries().await?;
        entries.retain(|(key, _)| key.starts_with(prefix));
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(entries)
      }
      Self::Worker {
        network,
        control_nodes,
      } => list_prefix_from_control_nodes(network, control_nodes, group_id, prefix).await,
    }
  }

  fn role_name(&self) -> &'static str {
    match self {
      Self::Control { .. } => "control",
      Self::Worker { .. } => "worker",
    }
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
      backend: self.backend.clone(),
      kv_client: self.kv_client.clone(),
      claimed: self.claimed.clone(),
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
    match &self.backend {
      RaftApalisBackend::Control { raft, .. } => {
        let metrics = raft.metrics().borrow_watched().clone();
        if metrics.state == ServerState::Leader {
          self.schedule_next_to_worker().await?;
        }
        Ok(None)
      }
      RaftApalisBackend::Worker { .. } => self.try_claim_assigned(worker_id).await,
    }
  }

  async fn schedule_next_to_worker(&self) -> Result<(), RaftApalisError> {
    self.requeue_expired_assignments().await?;
    let workers = self.active_workers().await?;
    if workers.is_empty() {
      return Ok(());
    }

    let prefix = task_key_prefix(self.queue.as_ref());
    let entries = self.entries_with_prefix(&prefix).await?;
    let now = current_unix_secs();
    for (key, value) in entries {
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

      let Some(target_worker) = select_worker_for_task(&workers, &record.task_id) else {
        return Ok(());
      };
      let target_worker_id = target_worker.node_id.clone();
      record.status = StoredStatus::Running;
      record.lock_by = None;
      record.assigned_node_id = Some(target_worker_id.clone());
      record.lease_epoch = Some(target_worker.lease_epoch);
      let task_id = record.task_id.clone();
      self.write_record(key, record).await?;
      tracing::debug!(
        task_id = %task_id,
        worker_node_id = %target_worker_id,
        lease_epoch = target_worker.lease_epoch,
        "scheduled apalis task to libp2p worker"
      );
      return Ok(());
    }

    Ok(())
  }

  async fn try_claim_assigned(
    &self,
    worker_id: &str,
  ) -> Result<Option<Task<Vec<u8>, RaftTaskContext, RaftTaskId>>, RaftApalisError> {
    let local_node_id = self.node_id.to_string();
    let prefix = task_key_prefix(self.queue.as_ref());
    let entries = self.entries_with_prefix(&prefix).await?;
    for (key, value) in entries {
      let mut record = match StoredTask::decode(&value) {
        Ok(record) => record,
        Err(err) => {
          tracing::warn!(%key, error = ?err, "skipping invalid apalis task record");
          continue;
        }
      };

      if record.status != StoredStatus::Running {
        continue;
      }
      if record.assigned_node_id.as_deref() != Some(local_node_id.as_str()) {
        continue;
      }
      if let Some(lock_by) = record.lock_by.as_deref()
        && lock_by != worker_id
      {
        continue;
      }

      let task_id = record.task_id.clone();
      if !self.insert_local_claim(&task_id).await {
        continue;
      }

      record.lock_by = Some(worker_id.to_string());
      let task = match record.clone().into_compact_task() {
        Ok(task) => task,
        Err(err) => {
          self.remove_local_claim(&task_id).await;
          return Err(err);
        }
      };

      if let Err(err) = self.write_record(key, record).await {
        self.remove_local_claim(&task_id).await;
        return Err(err);
      }

      return Ok(Some(task));
    }

    Ok(None)
  }

  async fn active_workers(&self) -> Result<Vec<WorkerRecord>, RaftApalisError> {
    let prefix = worker_key_prefix(self.queue.as_ref());
    let now = current_unix_secs();
    let mut workers = Vec::new();
    for (key, value) in self.entries_with_prefix(&prefix).await? {
      let worker = match WorkerRecord::decode(&value) {
        Ok(worker) => worker,
        Err(err) => {
          tracing::warn!(%key, error = ?err, "skipping invalid apalis worker record");
          continue;
        }
      };
      if worker.expires_at >= now {
        workers.push(worker);
      }
    }
    workers.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    workers.dedup_by(|a, b| a.node_id == b.node_id);
    Ok(workers)
  }

  async fn requeue_expired_assignments(&self) -> Result<(), RaftApalisError> {
    let active_workers = self.active_workers().await?;
    let active_worker_ids = active_workers
      .into_iter()
      .map(|worker| worker.node_id)
      .collect::<BTreeSet<_>>();
    let prefix = task_key_prefix(self.queue.as_ref());
    let entries = self.entries_with_prefix(&prefix).await?;
    for (key, value) in entries {
      let mut record = match StoredTask::decode(&value) {
        Ok(record) => record,
        Err(err) => {
          tracing::warn!(%key, error = ?err, "skipping invalid apalis task record");
          continue;
        }
      };

      if record.status != StoredStatus::Running {
        continue;
      }

      let Some(assigned_node_id) = record.assigned_node_id.as_deref() else {
        continue;
      };
      if active_worker_ids.contains(assigned_node_id) {
        continue;
      }

      let task_id = record.task_id.clone();
      let expired_worker = assigned_node_id.to_string();
      record.status = StoredStatus::Queued;
      record.lock_by = None;
      record.assigned_node_id = None;
      record.lease_epoch = None;
      self.write_record(key, record).await?;
      tracing::warn!(
        task_id = %task_id,
        worker_node_id = %expired_worker,
        "requeued apalis task assigned to inactive libp2p worker"
      );
    }

    Ok(())
  }

  async fn insert_local_claim(&self, task_id: &str) -> bool {
    self.claimed.lock().await.insert(task_id.to_string())
  }

  async fn remove_local_claim(&self, task_id: &str) {
    self.claimed.lock().await.remove(task_id);
  }
}

impl<Args, C> Clone for RaftApalisStorage<Args, C> {
  fn clone(&self) -> Self {
    Self {
      node_id: self.node_id.clone(),
      group_id: self.group_id.clone(),
      queue: self.queue.clone(),
      backend: self.backend.clone(),
      kv_client: self.kv_client.clone(),
      poll_interval: self.poll_interval,
      claimed: self.claimed.clone(),
      _args: PhantomData,
      _codec: PhantomData,
    }
  }
}

impl<Args, C> fmt::Debug for RaftApalisStorage<Args, C> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("RaftApalisStorage")
      .field("node_id", &self.node_id)
      .field("group_id", &self.group_id)
      .field("queue", &self.queue)
      .field("poll_interval", &self.poll_interval)
      .finish()
  }
}

#[derive(Clone)]
pub struct RaftApalisAck<C = SonicCodec<Vec<u8>>> {
  group_id: String,
  backend: RaftApalisBackend,
  kv_client: KvClient,
  claimed: Arc<Mutex<BTreeSet<String>>>,
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

    let task_id = task_id.to_string();
    let key = task_record_key(&self.group_id, &task_id);
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

    let backend = self.backend.clone();
    let kv_client = self.kv_client.clone();
    let group_id = self.group_id.clone();
    let claimed = self.claimed.clone();
    let lock_by = parts.ctx.lock_by.clone();
    let assigned_node_id = parts.ctx.assigned_node_id.clone();
    let lease_epoch = parts.ctx.lease_epoch;
    let attempt = parts.attempt.current();
    let run_at = parts.run_at;
    let idempotency_key = parts.idempotency_key.clone();

    async move {
      let result = result?;
      let mut entries = backend.entries_with_prefix(&group_id, &key).await?;
      let Some((_, current_value)) = entries.drain(..).find(|(entry_key, _)| entry_key == &key)
      else {
        claimed.lock().await.remove(&task_id);
        return Err(RaftApalisError::new(format!(
          "task record disappeared before ack: {task_id}"
        )));
      };
      let mut record = StoredTask::decode(&current_value)?;
      if record.assigned_node_id != assigned_node_id || record.lease_epoch != lease_epoch {
        tracing::warn!(
          task_id = %task_id,
          ack_assigned_node_id = ?assigned_node_id,
          ack_lease_epoch = ?lease_epoch,
          current_assigned_node_id = ?record.assigned_node_id,
          current_lease_epoch = ?record.lease_epoch,
          "ignored stale apalis task ack"
        );
        claimed.lock().await.remove(&task_id);
        return Ok(());
      }

      record.attempts = attempt;
      record.status = status;
      record.run_at = run_at;
      record.idempotency_key = idempotency_key;
      record.lock_by = lock_by;
      record.assigned_node_id = assigned_node_id;
      record.lease_epoch = lease_epoch;
      record.result = Some(result);
      let value = sonic_rs::to_string(&record)?;
      backend.write_raw(&kv_client, group_id, key, value).await?;
      claimed.lock().await.remove(&task_id);
      Ok(())
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

async fn write_raw_to_control_nodes(
  network: &Libp2pNetworkFactory,
  control_nodes: &[NodeId],
  group_id: String,
  key: String,
  value: String,
) -> Result<(), RaftApalisError> {
  let mut last_error = None;
  for node_id in control_nodes {
    let response = network
      .request_kv(
        node_id.clone(),
        RaftKvRequest {
          group_id: group_id.clone(),
          op: Some(KvRequestOp::Set(SetValueRequest {
            key: key.clone(),
            value: value.clone(),
          })),
        },
      )
      .await;

    match response {
      Ok(response) => match response.op {
        Some(KvResponseOp::Set(resp)) if resp.ok => return Ok(()),
        Some(KvResponseOp::Error(ErrorResponse { message })) => {
          last_error = Some(message);
        }
        other => {
          last_error = Some(format!("unexpected raft kv response: {other:?}"));
        }
      },
      Err(err) => {
        last_error = Some(format!("{err}"));
      }
    }
  }

  Err(RaftApalisError::new(format!(
    "write to control plane failed: {}",
    last_error.unwrap_or_else(|| "no control nodes configured".to_string())
  )))
}

async fn list_prefix_from_control_nodes(
  network: &Libp2pNetworkFactory,
  control_nodes: &[NodeId],
  group_id: &str,
  prefix: &str,
) -> Result<Vec<(String, String)>, RaftApalisError> {
  let mut last_error = None;
  for node_id in control_nodes {
    let response = network
      .request_kv(
        node_id.clone(),
        RaftKvRequest {
          group_id: group_id.to_string(),
          op: Some(KvRequestOp::ListPrefix(ListPrefixRequest {
            prefix: prefix.to_string(),
          })),
        },
      )
      .await;

    match response {
      Ok(response) => match response.op {
        Some(KvResponseOp::ListPrefix(resp)) => {
          let mut entries = resp
            .entries
            .into_iter()
            .map(|entry| (entry.key, entry.value))
            .collect::<Vec<_>>();
          entries.sort_by(|a, b| a.0.cmp(&b.0));
          return Ok(entries);
        }
        Some(KvResponseOp::Error(ErrorResponse { message })) => {
          last_error = Some(message);
        }
        other => {
          last_error = Some(format!("unexpected raft kv response: {other:?}"));
        }
      },
      Err(err) => {
        last_error = Some(format!("{err}"));
      }
    }
  }

  Err(RaftApalisError::new(format!(
    "read from control plane failed: {}",
    last_error.unwrap_or_else(|| "no control nodes configured".to_string())
  )))
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
  #[serde(default)]
  assigned_node_id: Option<String>,
  #[serde(default)]
  lease_epoch: Option<u64>,
  result: Option<TaskResultRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskRecordView {
  pub task_id: String,
  pub status: String,
  pub attempts: usize,
  pub run_at: u64,
  pub lock_by: Option<String>,
  pub assigned_node_id: Option<String>,
  pub lease_epoch: Option<u64>,
  pub result_ok: Option<bool>,
  pub error: Option<String>,
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
      assigned_node_id: task.parts.ctx.assigned_node_id,
      lease_epoch: task.parts.ctx.lease_epoch,
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
        assigned_node_id: self.assigned_node_id,
        lease_epoch: self.lease_epoch,
      });
    if let Some(idempotency_key) = self.idempotency_key {
      task = task.with_idempotency_key(idempotency_key);
    }
    Ok(task.build())
  }

  fn decode(value: &str) -> Result<Self, RaftApalisError> {
    sonic_rs::from_str(value).map_err(Into::into)
  }

  fn view(self) -> TaskRecordView {
    let result_ok = self.result.as_ref().map(|result| result.ok);
    let error = self.result.and_then(|result| result.error);
    TaskRecordView {
      task_id: self.task_id,
      status: self.status.as_str().to_string(),
      attempts: self.attempts,
      run_at: self.run_at,
      lock_by: self.lock_by,
      assigned_node_id: self.assigned_node_id,
      lease_epoch: self.lease_epoch,
      result_ok,
      error,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskResultRecord {
  ok: bool,
  payload: Vec<u8>,
  error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerRecord {
  pub node_id: String,
  pub worker_name: String,
  pub lease_epoch: u64,
  pub expires_at: u64,
}

impl WorkerRecord {
  fn decode(value: &str) -> Result<Self, RaftApalisError> {
    sonic_rs::from_str(value).map_err(Into::into)
  }
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

impl StoredStatus {
  fn as_str(self) -> &'static str {
    match self {
      Self::Pending => "pending",
      Self::Queued => "queued",
      Self::Running => "running",
      Self::Done => "done",
      Self::Failed => "failed",
      Self::Killed => "killed",
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

impl From<sonic_rs::Error> for RaftApalisError {
  fn from(value: sonic_rs::Error) -> Self {
    Self::new(value.to_string())
  }
}

pub async fn send_email(task: Email) -> Result<(), BoxDynError> {
  tracing::info!(to = %task.to, "sending email");
  Ok(())
}

pub fn build_email_storage(
  node_id: NodeId,
  group_id: impl Into<String>,
  group: GroupHandle,
  kv_client: KvClient,
) -> RaftApalisStorage<Email> {
  RaftApalisStorage::new(node_id, group_id, group, kv_client)
}

pub fn build_worker_email_storage(
  node_id: NodeId,
  group_id: impl Into<String>,
  network: Libp2pNetworkFactory,
  control_nodes: Vec<NodeId>,
  kv_client: KvClient,
) -> RaftApalisStorage<Email> {
  RaftApalisStorage::worker(node_id, group_id, network, control_nodes, kv_client)
}

pub async fn run_email_worker(
  worker_name: impl AsRef<str>,
  storage: RaftApalisStorage<Email>,
  mut shutdown_rx: crate::signal::ShutdownRx,
) -> anyhow::Result<()> {
  let worker_name = worker_name.as_ref().to_string();
  let lease_handle = match &storage.backend {
    RaftApalisBackend::Worker { .. } => {
      let storage = storage.clone();
      let worker_name = worker_name.clone();
      let shutdown_rx = shutdown_rx.clone();
      Some(tokio::spawn(async move {
        run_worker_lease_renewal(worker_name, storage, shutdown_rx).await
      }))
    }
    RaftApalisBackend::Control { .. } => None,
  };

  let worker = WorkerBuilder::new(worker_name)
    .backend(storage)
    .build(send_email);
  let result = worker
    .run_until(async move {
      let _ = shutdown_rx.changed().await;
      Ok::<(), apalis::prelude::WorkerError>(())
    })
    .await
    .map_err(|err| anyhow::anyhow!("apalis email worker failed: {err}"));

  if let Some(handle) = lease_handle {
    handle.abort();
  }

  result
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

pub fn worker_key_prefix(queue: &str) -> String {
  format!("apalis:{queue}:{WORKER_KEY_PART}:")
}

pub fn worker_record_key(queue: &str, node_id: &str) -> String {
  format!("{}{node_id}", worker_key_prefix(queue))
}

async fn run_worker_lease_renewal(
  worker_name: String,
  storage: RaftApalisStorage<Email>,
  mut shutdown_rx: crate::signal::ShutdownRx,
) {
  let mut lease_epoch = current_unix_secs();
  if let Err(err) = renew_worker_lease(&storage, &worker_name, lease_epoch).await {
    tracing::warn!(
      worker_name = %worker_name,
      role = storage.backend.role_name(),
      error = ?err,
      "initial apalis worker lease registration failed"
    );
  }

  let mut tick = tokio::time::interval(WORKER_LEASE_INTERVAL);
  tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!(worker_name = %worker_name, "stopping apalis worker lease renewal");
        break;
      }
      _ = tick.tick() => {
        lease_epoch = lease_epoch.saturating_add(1);
        if let Err(err) = renew_worker_lease(&storage, &worker_name, lease_epoch).await {
          tracing::warn!(
            worker_name = %worker_name,
            role = storage.backend.role_name(),
            error = ?err,
            "apalis worker lease renewal failed"
          );
        }
      }
    }
  }
}

async fn renew_worker_lease(
  storage: &RaftApalisStorage<Email>,
  worker_name: &str,
  lease_epoch: u64,
) -> Result<(), RaftApalisError> {
  let node_id = storage.node_id.to_string();
  let record = WorkerRecord {
    node_id: node_id.clone(),
    worker_name: worker_name.to_string(),
    lease_epoch,
    expires_at: current_unix_secs().saturating_add(WORKER_LEASE_TTL.as_secs()),
  };
  let key = worker_record_key(storage.queue.as_ref(), &node_id);
  let value = sonic_rs::to_string(&record)?;
  storage.write_raw(key, value).await
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

fn select_worker_for_task(workers: &[WorkerRecord], task_id: &str) -> Option<WorkerRecord> {
  if workers.is_empty() {
    return None;
  }

  let mut hasher = DefaultHasher::new();
  task_id.hash(&mut hasher);
  let index = (hasher.finish() as usize) % workers.len();
  workers.get(index).cloned()
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
        assigned_node_id: Some("node-b".to_string()),
        lease_epoch: Some(7),
      })
      .with_idempotency_key("idem-1")
      .build();

    let record = StoredTask::from_compact_task(task).expect("stored task");
    let encoded = sonic_rs::to_string(&record).expect("encode");
    let decoded = StoredTask::decode(&encoded).expect("decode");
    let task = decoded.into_compact_task().expect("compact task");

    assert_eq!(task.args, vec![1, 2, 3]);
    assert_eq!(task.parts.task_id.expect("task id").to_string(), "task-1");
    assert_eq!(task.parts.attempt.current(), 2);
    assert_eq!(task.parts.status.load(), Status::Queued);
    assert_eq!(task.parts.run_at, 123);
    assert_eq!(task.parts.ctx.lock_by, Some("worker-a".to_string()));
    assert_eq!(task.parts.ctx.assigned_node_id, Some("node-b".to_string()));
    assert_eq!(task.parts.ctx.lease_epoch, Some(7));
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
    assert_eq!(
      worker_record_key("apalis", "worker-1"),
      "apalis:apalis:worker:worker-1".to_string()
    );
  }

  #[test]
  fn select_worker_for_task_returns_a_stable_worker() {
    let workers = vec![
      WorkerRecord {
        node_id: "worker-a".to_string(),
        worker_name: "worker-a".to_string(),
        lease_epoch: 1,
        expires_at: 10,
      },
      WorkerRecord {
        node_id: "worker-b".to_string(),
        worker_name: "worker-b".to_string(),
        lease_epoch: 1,
        expires_at: 10,
      },
      WorkerRecord {
        node_id: "worker-c".to_string(),
        worker_name: "worker-c".to_string(),
        lease_epoch: 1,
        expires_at: 10,
      },
    ];

    let picked = select_worker_for_task(&workers, "task-1").expect("worker");
    assert!(
      workers
        .iter()
        .any(|worker| worker.node_id == picked.node_id)
    );
    assert_eq!(
      select_worker_for_task(&workers, "task-1").map(|worker| worker.node_id),
      Some(picked.node_id)
    );
    assert!(select_worker_for_task(&[], "task-1").is_none());
  }
}

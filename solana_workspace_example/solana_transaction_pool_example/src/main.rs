use std::{
  cmp::Reverse,
  env, fs,
  str::FromStr,
  sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
  },
  time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use deadpool::managed::{Manager, Metrics, Pool, RecycleResult};
use priority_queue::PriorityQueue;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
  native_token::LAMPORTS_PER_SOL,
  signature::{Signature, Signer},
  signer::keypair::Keypair,
  transaction::{Transaction, VersionedTransaction},
};
use solana_system_interface::instruction as system_instruction;
use tokio::{
  sync::{oneshot, Mutex, Notify},
  task::JoinSet,
  time::{interval, sleep, sleep_until, timeout, Instant as TokioInstant},
};

#[derive(Debug, Clone)]
struct Config {
  rpc_url: String,
  send_workers: usize,
  confirm_workers: usize,
  queue_size: usize,
  rpc_pool_size: usize,
  tx_file: Option<String>,
  demo_count: usize,
  send_timeout_ms: u64,
  confirm_timeout_ms: u64,
  confirm_rpc_timeout_ms: u64,
  confirm_max_retries: usize,
  confirm_initial_delay_ms: u64,
  confirm_max_delay_ms: u64,
  worker_idle_tick_ms: u64,
  trace_workers: bool,
}

impl Config {
  fn from_env() -> Self {
    let idle_tick_ms = env_u64("WORKER_IDLE_TICK_MS", 5_000);
    let worker_idle_tick_ms = if idle_tick_ms == 0 {
      5_000
    } else {
      idle_tick_ms
    };
    let send_workers = env_usize("SEND_WORKERS", 4);
    let confirm_workers = env_usize("CONFIRM_WORKERS", 4);
    let rpc_pool_size = env_usize(
      "RPC_POOL_SIZE",
      send_workers.saturating_add(confirm_workers),
    )
    .max(1);

    Self {
      rpc_url: env::var("RPC_URL").unwrap_or_else(|_| "http://localhost:8899".to_string()),
      send_workers,
      confirm_workers,
      queue_size: env_usize("QUEUE_SIZE", 256),
      rpc_pool_size,
      tx_file: env::var("TX_FILE").ok(),
      demo_count: env_usize("DEMO_COUNT", 4),
      send_timeout_ms: env_u64("SEND_TIMEOUT_MS", 5_000),
      confirm_timeout_ms: env_u64("CONFIRM_TIMEOUT_MS", 30_000),
      confirm_rpc_timeout_ms: env_u64("CONFIRM_RPC_TIMEOUT_MS", 2_000),
      confirm_max_retries: env_usize("CONFIRM_MAX_RETRIES", 20),
      confirm_initial_delay_ms: env_u64("CONFIRM_INITIAL_DELAY_MS", 200),
      confirm_max_delay_ms: env_u64("CONFIRM_MAX_DELAY_MS", 4000),
      worker_idle_tick_ms,
      trace_workers: env_bool("TRACE_WORKER", false),
    }
  }
}

#[derive(Clone)]
struct RpcClientManager {
  rpc_url: String,
  commitment: CommitmentConfig,
}

impl RpcClientManager {
  fn new(rpc_url: String, commitment: CommitmentConfig) -> Self {
    Self {
      rpc_url,
      commitment,
    }
  }
}

impl Manager for RpcClientManager {
  type Type = RpcClient;
  type Error = anyhow::Error;

  fn create(&self) -> impl std::future::Future<Output = Result<Self::Type, Self::Error>> + Send {
    let rpc_url = self.rpc_url.clone();
    let commitment = self.commitment.clone();
    async move { Ok(RpcClient::new_with_commitment(rpc_url, commitment)) }
  }

  fn recycle(
    &self,
    _obj: &mut Self::Type,
    _metrics: &Metrics,
  ) -> impl std::future::Future<Output = RecycleResult<Self::Error>> + Send {
    async { Ok(()) }
  }
}

type RpcPool = Pool<RpcClientManager>;

#[derive(Debug, Clone)]
struct SendRequest {
  request_id: String,
  tx_b64: String,
  priority: Priority,
}

#[derive(Debug)]
struct SendResponse {
  request_id: String,
  signature: String,
  worker_id: usize,
  elapsed_ms: u128,
  priority: Priority,
}

struct SendTask {
  req: SendRequest,
  resp: oneshot::Sender<Result<SendResponse, String>>,
}

#[derive(Debug)]
struct ConfirmRequest {
  request_id: String,
  signature: Signature,
  priority: Priority,
}

#[derive(Debug)]
struct ConfirmResponse {
  request_id: String,
  signature: String,
  confirmed: bool,
  reason: Option<String>,
  worker_id: usize,
  attempts: usize,
  elapsed_ms: u128,
  priority: Priority,
}

struct ConfirmTask {
  req: ConfirmRequest,
  resp: oneshot::Sender<ConfirmResponse>,
}

#[async_trait]
trait SendInterface {
  fn id(&self) -> usize;
  async fn send_request(&self, req: SendRequest, started: Instant) -> Result<SendResponse, String>;
}

struct SendWorker {
  id: usize,
  pool: RpcPool,
  timeout: Duration,
  trace: bool,
}

#[async_trait]
impl SendInterface for SendWorker {
  fn id(&self) -> usize {
    self.id
  }

  async fn send_request(&self, req: SendRequest, started: Instant) -> Result<SendResponse, String> {
    let tx = decode_transaction(&req.tx_b64)?;
    let client = self
      .pool
      .get()
      .await
      .map_err(|err| format!("rpc pool error: {err}"))?;
    let send_fut = client.send_transaction(&tx);
    let signature = match timeout(self.timeout, send_fut).await {
      Ok(Ok(signature)) => signature,
      Ok(Err(err)) => return Err(format!("rpc send error: {err}")),
      Err(_) => {
        eprintln!(
          "send timeout worker_id={} request_id={} timeout_ms={}",
          self.id,
          req.request_id,
          self.timeout.as_millis()
        );
        return Err(format!(
          "send timeout after {} ms",
          self.timeout.as_millis()
        ));
      }
    };

    if self.trace {
      eprintln!(
        "send ok worker_id={} request_id={}",
        self.id, req.request_id
      );
    }

    Ok(SendResponse {
      request_id: req.request_id,
      signature: signature.to_string(),
      worker_id: self.id,
      elapsed_ms: started.elapsed().as_millis(),
      priority: req.priority,
    })
  }
}

#[async_trait]
trait ConfirmInterface {
  fn id(&self) -> usize;
  async fn confirm_request(&self, req: ConfirmRequest, started: Instant) -> ConfirmResponse;
}

struct ConfirmWorker {
  id: usize,
  pool: RpcPool,
  timeout: Duration,
  rpc_timeout: Duration,
  max_retries: usize,
  initial_delay: Duration,
  max_delay: Duration,
  commitment: CommitmentConfig,
  trace: bool,
}

#[async_trait]
impl ConfirmInterface for ConfirmWorker {
  fn id(&self) -> usize {
    self.id
  }

  async fn confirm_request(&self, req: ConfirmRequest, started: Instant) -> ConfirmResponse {
    let deadline = TokioInstant::now() + self.timeout;
    let mut delay = self.initial_delay;
    let mut attempts = 0;

    loop {
      if TokioInstant::now() >= deadline {
        eprintln!(
          "confirm timeout worker_id={} request_id={} signature={}",
          self.id, req.request_id, req.signature
        );
        return ConfirmResponse {
          request_id: req.request_id,
          signature: req.signature.to_string(),
          confirmed: false,
          reason: Some(format!("timeout after {} ms", self.timeout.as_millis())),
          worker_id: self.id,
          attempts,
          elapsed_ms: started.elapsed().as_millis(),
          priority: req.priority,
        };
      }

      attempts += 1;
      let remaining = deadline.saturating_duration_since(TokioInstant::now());
      let call_timeout = self.rpc_timeout.min(remaining);

      let status_result = {
        let client = match self.pool.get().await {
          Ok(client) => client,
          Err(err) => {
            return ConfirmResponse {
              request_id: req.request_id,
              signature: req.signature.to_string(),
              confirmed: false,
              reason: Some(format!("rpc pool error: {err}")),
              worker_id: self.id,
              attempts,
              elapsed_ms: started.elapsed().as_millis(),
              priority: req.priority,
            };
          }
        };

        timeout(
          call_timeout,
          client.get_signature_status_with_commitment(&req.signature, self.commitment),
        )
        .await
      };

      match status_result {
        Ok(Ok(Some(Ok(())))) => {
          if self.trace {
            eprintln!(
              "confirm ok worker_id={} request_id={}",
              self.id, req.request_id
            );
          }
          return ConfirmResponse {
            request_id: req.request_id,
            signature: req.signature.to_string(),
            confirmed: true,
            reason: None,
            worker_id: self.id,
            attempts,
            elapsed_ms: started.elapsed().as_millis(),
            priority: req.priority,
          };
        }
        Ok(Ok(Some(Err(err)))) => {
          return ConfirmResponse {
            request_id: req.request_id,
            signature: req.signature.to_string(),
            confirmed: false,
            reason: Some(format!("transaction error: {err:?}")),
            worker_id: self.id,
            attempts,
            elapsed_ms: started.elapsed().as_millis(),
            priority: req.priority,
          };
        }
        Ok(Ok(None)) => {
          if attempts >= self.max_retries {
            return ConfirmResponse {
              request_id: req.request_id,
              signature: req.signature.to_string(),
              confirmed: false,
              reason: Some(format!("timeout after {attempts} attempts")),
              worker_id: self.id,
              attempts,
              elapsed_ms: started.elapsed().as_millis(),
              priority: req.priority,
            };
          }
        }
        Ok(Err(err)) => {
          return ConfirmResponse {
            request_id: req.request_id,
            signature: req.signature.to_string(),
            confirmed: false,
            reason: Some(format!("rpc status error: {err}")),
            worker_id: self.id,
            attempts,
            elapsed_ms: started.elapsed().as_millis(),
            priority: req.priority,
          };
        }
        Err(_) => {
          eprintln!(
            "confirm rpc timeout worker_id={} request_id={} signature={} timeout_ms={}",
            self.id,
            req.request_id,
            req.signature,
            call_timeout.as_millis()
          );
          return ConfirmResponse {
            request_id: req.request_id,
            signature: req.signature.to_string(),
            confirmed: false,
            reason: Some(format!("rpc timeout after {} ms", call_timeout.as_millis())),
            worker_id: self.id,
            attempts,
            elapsed_ms: started.elapsed().as_millis(),
            priority: req.priority,
          };
        }
      }

      let sleep_for = delay.min(self.max_delay);
      tokio::select! {
        _ = sleep(sleep_for) => {},
        _ = sleep_until(deadline) => {
          eprintln!(
            "confirm timeout worker_id={} request_id={} signature={}",
            self.id,
            req.request_id,
            req.signature
          );
          return ConfirmResponse {
            request_id: req.request_id,
            signature: req.signature.to_string(),
            confirmed: false,
            reason: Some(format!(
              "timeout after {} ms",
              self.timeout.as_millis()
            )),
            worker_id: self.id,
            attempts,
            elapsed_ms: started.elapsed().as_millis(),
            priority: req.priority,
          };
        }
      }

      let next_ms = delay.as_millis().saturating_mul(2) as u64;
      let cap_ms = self.max_delay.as_millis() as u64;
      delay = Duration::from_millis(next_ms.min(cap_ms));
    }
  }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum Priority {
  High,
  Normal,
  Low,
}

impl Priority {
  fn parse(value: &str) -> Option<Self> {
    match value.trim().to_lowercase().as_str() {
      "high" | "h" => Some(Self::High),
      "low" | "l" => Some(Self::Low),
      "normal" | "n" | "medium" | "m" => Some(Self::Normal),
      _ => None,
    }
  }

  fn label(self) -> &'static str {
    match self {
      Self::High => "high",
      Self::Normal => "normal",
      Self::Low => "low",
    }
  }
}

impl std::fmt::Display for Priority {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.label())
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct PriorityKey {
  level: u8,
  seq: Reverse<u64>,
}

#[derive(Debug)]
struct QueueItem<T> {
  id: u64,
  task: T,
}

impl<T> PartialEq for QueueItem<T> {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl<T> Eq for QueueItem<T> {}

impl<T> std::hash::Hash for QueueItem<T> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.id.hash(state);
  }
}

struct WorkerQueue<T> {
  inner: Mutex<PriorityQueue<QueueItem<T>, PriorityKey>>,
  notify: Arc<Notify>,
  max_len: usize,
  seq: AtomicU64,
  counters: Arc<QueueCounters>,
}

struct ShardedQueue<T> {
  queues: Vec<Arc<WorkerQueue<T>>>,
  next_idx: AtomicUsize,
  steal_seq: AtomicUsize,
  notify: Arc<Notify>,
}

struct QueueCounters {
  len: AtomicUsize,
  drops: AtomicU64,
}

impl QueueCounters {
  fn new() -> Self {
    Self {
      len: AtomicUsize::new(0),
      drops: AtomicU64::new(0),
    }
  }
}
impl<T> WorkerQueue<T> {
  fn new(max_len: usize, counters: Arc<QueueCounters>) -> Self {
    Self {
      inner: Mutex::new(PriorityQueue::new()),
      notify: Arc::new(Notify::new()),
      max_len,
      seq: AtomicU64::new(0),
      counters,
    }
  }

  async fn try_pop(&self) -> Option<T> {
    let mut guard = self.inner.lock().await;
    let popped = guard.pop();
    drop(guard);

    if let Some((item, _priority)) = popped {
      self.counters.len.fetch_sub(1, Ordering::Relaxed);
      return Some(item.task);
    }

    None
  }
  async fn push(&self, priority: Priority, item: T) -> Result<(), String> {
    let prev = self.counters.len.fetch_add(1, Ordering::Relaxed);
    if prev >= self.max_len {
      self.counters.len.fetch_sub(1, Ordering::Relaxed);
      self.counters.drops.fetch_add(1, Ordering::Relaxed);
      return Err("priority queue is full".to_string());
    }

    let level = match priority {
      Priority::High => 2,
      Priority::Normal => 1,
      Priority::Low => 0,
    };
    let id = self.seq.fetch_add(1, Ordering::Relaxed);
    let key = PriorityKey {
      level,
      seq: Reverse(id),
    };
    let mut guard = self.inner.lock().await;
    guard.push(QueueItem { id, task: item }, key);
    drop(guard);
    self.notify.notify_one();
    Ok(())
  }
}

impl<T> ShardedQueue<T>
where
  T: Send,
{
  fn new(worker_count: usize, max_len: usize) -> Self {
    let worker_count = worker_count.max(1);
    let counters = Arc::new(QueueCounters::new());
    let queues = (0 .. worker_count)
      .map(|_| Arc::new(WorkerQueue::new(max_len, Arc::clone(&counters))))
      .collect();
    Self {
      queues,
      next_idx: AtomicUsize::new(0),
      steal_seq: AtomicUsize::new(0),
      notify: Arc::new(Notify::new()),
    }
  }

  fn local_notify(&self, worker_id: usize) -> Arc<Notify> {
    self.queues[worker_id].notify.clone()
  }

  fn global_notify(&self) -> Arc<Notify> {
    self.notify.clone()
  }

  async fn push(&self, priority: Priority, item: T) -> Result<(), String> {
    let idx = self.next_idx.fetch_add(1, Ordering::Relaxed) % self.queues.len();
    self.queues[idx].push(priority, item).await?;
    self.notify.notify_one();
    Ok(())
  }

  async fn try_pop(&self, worker_id: usize) -> Option<T> {
    self.queues[worker_id].try_pop().await
  }

  async fn steal(&self, worker_id: usize) -> Option<T> {
    let total = self.queues.len();
    if total <= 1 {
      return None;
    }
    let start = self.steal_seq.fetch_add(1, Ordering::Relaxed) % total;
    for offset in 0 .. total {
      let idx = (start + offset) % total;
      if idx == worker_id {
        continue;
      }
      if let Some(task) = self.queues[idx].try_pop().await {
        return Some(task);
      }
    }
    None
  }
}

fn init_queues(
  cfg: &Config,
  send_workers: usize,
  confirm_workers: usize,
) -> (Arc<ShardedQueue<SendTask>>, Arc<ShardedQueue<ConfirmTask>>) {
  let max_len = cfg.queue_size.max(1);
  let send_queue = Arc::new(ShardedQueue::<SendTask>::new(send_workers, max_len));

  let confirm_queue = Arc::new(ShardedQueue::<ConfirmTask>::new(confirm_workers, max_len));

  (send_queue, confirm_queue)
}

fn build_rpc_pool(cfg: &Config) -> Result<RpcPool> {
  let manager = RpcClientManager::new(cfg.rpc_url.clone(), CommitmentConfig::confirmed());
  Pool::builder(manager)
    .max_size(cfg.rpc_pool_size)
    .build()
    .map_err(|err| anyhow!("rpc pool build error: {err}"))
}

struct SendPool {
  queue: Arc<ShardedQueue<SendTask>>,
}

impl SendPool {
  fn new(
    worker_count: usize,
    cfg: &Config,
    queue: Arc<ShardedQueue<SendTask>>,
    rpc_pool: RpcPool,
  ) -> Self {
    for id in 0 .. worker_count {
      let in_flight = Arc::new(AtomicUsize::new(0));
      let worker = SendWorker {
        id,
        pool: rpc_pool.clone(),
        timeout: Duration::from_millis(cfg.send_timeout_ms.max(1)),
        trace: cfg.trace_workers,
      };
      let in_flight_clone = Arc::clone(&in_flight);
      let idle_tick = Duration::from_millis(cfg.worker_idle_tick_ms);
      let trace = cfg.trace_workers;
      let queue_clone = Arc::clone(&queue);

      tokio::spawn(async move {
        send_worker_loop(worker, queue_clone, in_flight_clone, idle_tick, trace).await;
      });
    }

    Self { queue }
  }

  async fn submit(&self, req: SendRequest) -> Result<SendResponse, String> {
    let (resp_tx, resp_rx) = oneshot::channel();
    let priority = req.priority;
    let task = SendTask { req, resp: resp_tx };

    self
      .queue
      .push(priority, task)
      .await
      .map_err(|e| format!("send queue error: {e}"))?;

    resp_rx
      .await
      .map_err(|_| "send worker dropped".to_string())?
  }
}

struct ConfirmPool {
  queue: Arc<ShardedQueue<ConfirmTask>>,
}

impl ConfirmPool {
  fn new(
    worker_count: usize,
    cfg: &Config,
    queue: Arc<ShardedQueue<ConfirmTask>>,
    rpc_pool: RpcPool,
  ) -> Self {
    for id in 0 .. worker_count {
      let in_flight = Arc::new(AtomicUsize::new(0));
      let worker = ConfirmWorker {
        id,
        pool: rpc_pool.clone(),
        timeout: Duration::from_millis(cfg.confirm_timeout_ms.max(1)),
        rpc_timeout: Duration::from_millis(cfg.confirm_rpc_timeout_ms.max(1)),
        max_retries: cfg.confirm_max_retries,
        initial_delay: Duration::from_millis(cfg.confirm_initial_delay_ms),
        max_delay: Duration::from_millis(cfg.confirm_max_delay_ms),
        commitment: CommitmentConfig::confirmed(),
        trace: cfg.trace_workers,
      };
      let in_flight_clone = Arc::clone(&in_flight);
      let idle_tick = Duration::from_millis(cfg.worker_idle_tick_ms);
      let trace = cfg.trace_workers;
      let queue_clone = Arc::clone(&queue);

      tokio::spawn(async move {
        confirm_worker_loop(worker, queue_clone, in_flight_clone, idle_tick, trace).await;
      });
    }

    Self { queue }
  }

  async fn submit(&self, req: ConfirmRequest) -> Result<ConfirmResponse, String> {
    let (resp_tx, resp_rx) = oneshot::channel();
    let priority = req.priority;
    let task = ConfirmTask { req, resp: resp_tx };

    self
      .queue
      .push(priority, task)
      .await
      .map_err(|e| format!("confirm queue error: {e}"))?;

    Ok(
      resp_rx
        .await
        .map_err(|_| "confirm worker dropped".to_string())?,
    )
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  let cfg = Config::from_env();
  let send_workers = cfg.send_workers.max(1);
  let confirm_workers = cfg.confirm_workers.max(1);

  println!(
    "rpc_url={}, send_workers={}, confirm_workers={}, queue_size={}, rpc_pool_size={}",
    cfg.rpc_url, send_workers, confirm_workers, cfg.queue_size, cfg.rpc_pool_size
  );

  let (send_queue, confirm_queue) = init_queues(&cfg, send_workers, confirm_workers);
  let rpc_pool = build_rpc_pool(&cfg)?;
  let send_pool = Arc::new(SendPool::new(
    send_workers,
    &cfg,
    send_queue,
    rpc_pool.clone(),
  ));
  let confirm_pool = Arc::new(ConfirmPool::new(
    confirm_workers,
    &cfg,
    confirm_queue,
    rpc_pool.clone(),
  ));

  let requests = if let Some(path) = &cfg.tx_file {
    load_requests_from_file(path)?
  } else {
    build_demo_requests(&rpc_pool, cfg.demo_count).await?
  };

  let mut join_set = JoinSet::new();
  for req in requests {
    let send_pool = Arc::clone(&send_pool);
    let confirm_pool = Arc::clone(&confirm_pool);

    join_set.spawn(async move {
      let send_res = send_pool.submit(req).await;
      match send_res {
        Ok(sent) => {
          println!(
            "sent request_id={}, signature={}, worker={}, elapsed_ms={}, priority={}",
            sent.request_id, sent.signature, sent.worker_id, sent.elapsed_ms, sent.priority
          );
          let signature = Signature::from_str(&sent.signature)
            .map_err(|e| anyhow!("invalid signature {}: {e}", sent.signature))?;
          let confirm_req = ConfirmRequest {
            request_id: sent.request_id.clone(),
            signature,
            priority: sent.priority,
          };
          let confirm_res = confirm_pool
            .submit(confirm_req)
            .await
            .map_err(|e| anyhow!("confirm error for {}: {e}", sent.request_id))?;

          Ok::<ConfirmResponse, anyhow::Error>(confirm_res)
        }
        Err(err) => Err(anyhow!("send error: {err}")),
      }
    });
  }

  while let Some(result) = join_set.join_next().await {
    match result {
      Ok(Ok(confirm)) => {
        if confirm.confirmed {
          println!(
            "confirmed request_id={}, signature={}, worker={}, attempts={}, elapsed_ms={}, \
             priority={}",
            confirm.request_id,
            confirm.signature,
            confirm.worker_id,
            confirm.attempts,
            confirm.elapsed_ms,
            confirm.priority
          );
        } else {
          println!(
            "failed request_id={}, signature={}, worker={}, attempts={}, elapsed_ms={}, \
             reason={}, priority={}",
            confirm.request_id,
            confirm.signature,
            confirm.worker_id,
            confirm.attempts,
            confirm.elapsed_ms,
            confirm.reason.unwrap_or_else(|| "unknown".to_string()),
            confirm.priority
          );
        }
      }
      Ok(Err(err)) => {
        eprintln!("task error: {err}");
      }
      Err(join_err) => {
        eprintln!("join error: {join_err}");
      }
    }
  }

  Ok(())
}

async fn send_worker_loop<W>(
  worker: W,
  queue: Arc<ShardedQueue<SendTask>>,
  in_flight: Arc<AtomicUsize>,
  idle_tick: Duration,
  trace: bool,
) where
  W: SendInterface + Send + Sync,
{
  let mut ticker = interval(idle_tick);
  let worker_id = worker.id();
  let local_notify = queue.local_notify(worker_id);
  let global_notify = queue.global_notify();
  loop {
    if let Some(task) = queue.try_pop(worker_id).await {
      in_flight.fetch_add(1, Ordering::Relaxed);
      let started = Instant::now();

      let result = worker.send_request(task.req, started).await;

      in_flight.fetch_sub(1, Ordering::Relaxed);
      let _ = task.resp.send(result);
      continue;
    }

    if let Some(task) = queue.steal(worker_id).await {
      in_flight.fetch_add(1, Ordering::Relaxed);
      let started = Instant::now();

      let result = worker.send_request(task.req, started).await;

      in_flight.fetch_sub(1, Ordering::Relaxed);
      let _ = task.resp.send(result);
      continue;
    }

    tokio::select! {
      _ = local_notify.notified() => {},
      _ = global_notify.notified() => {},
      _ = ticker.tick() => {
        if trace {
          eprintln!(
            "send idle worker_id={} in_flight={}",
            worker_id,
            in_flight.load(Ordering::Relaxed)
          );
        }
      }
    }
  }
}

async fn confirm_worker_loop<W>(
  worker: W,
  queue: Arc<ShardedQueue<ConfirmTask>>,
  in_flight: Arc<AtomicUsize>,
  idle_tick: Duration,
  trace: bool,
) where
  W: ConfirmInterface + Send + Sync,
{
  let mut ticker = interval(idle_tick);
  let worker_id = worker.id();
  let local_notify = queue.local_notify(worker_id);
  let global_notify = queue.global_notify();
  loop {
    if let Some(task) = queue.try_pop(worker_id).await {
      in_flight.fetch_add(1, Ordering::Relaxed);
      let started = Instant::now();

      let response = worker.confirm_request(task.req, started).await;

      in_flight.fetch_sub(1, Ordering::Relaxed);
      let _ = task.resp.send(response);
      continue;
    }

    if let Some(task) = queue.steal(worker_id).await {
      in_flight.fetch_add(1, Ordering::Relaxed);
      let started = Instant::now();

      let response = worker.confirm_request(task.req, started).await;

      in_flight.fetch_sub(1, Ordering::Relaxed);
      let _ = task.resp.send(response);
      continue;
    }

    tokio::select! {
      _ = local_notify.notified() => {},
      _ = global_notify.notified() => {},
      _ = ticker.tick() => {
        if trace {
          eprintln!(
            "confirm idle worker_id={} in_flight={}",
            worker_id,
            in_flight.load(Ordering::Relaxed)
          );
        }
      }
    }
  }
}

fn decode_transaction(tx_b64: &str) -> Result<VersionedTransaction, String> {
  let bytes = general_purpose::STANDARD
    .decode(tx_b64)
    .map_err(|e| format!("base64 decode error: {e}"))?;
  bincode::deserialize::<VersionedTransaction>(&bytes)
    .map_err(|e| format!("bincode decode error: {e}"))
}

fn load_requests_from_file(path: &str) -> Result<Vec<SendRequest>> {
  let content = fs::read_to_string(path).with_context(|| format!("read TX_FILE {path}"))?;
  let mut requests = Vec::new();

  for (idx, line) in content.lines().enumerate() {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
      continue;
    }
    let mut parts = line.split(',').map(str::trim);
    let request_id = parts
      .next()
      .filter(|s| !s.is_empty())
      .ok_or_else(|| anyhow!("line {} missing request_id", idx + 1))?;
    let second = parts
      .next()
      .filter(|s| !s.is_empty())
      .ok_or_else(|| anyhow!("line {} missing base64 tx", idx + 1))?;
    let third = parts.next().filter(|s| !s.is_empty());

    let (priority, tx_b64) = if let Some(tx_b64) = third {
      let priority = Priority::parse(second)
        .ok_or_else(|| anyhow!("line {} invalid priority {}", idx + 1, second))?;
      (priority, tx_b64)
    } else {
      (Priority::Normal, second)
    };

    if parts.next().is_some() {
      return Err(anyhow!("line {} has too many fields", idx + 1));
    }

    requests.push(SendRequest {
      request_id: request_id.to_string(),
      tx_b64: tx_b64.to_string(),
      priority,
    });
  }

  if requests.is_empty() {
    return Err(anyhow!("TX_FILE has no valid requests"));
  }

  Ok(requests)
}

async fn build_demo_requests(pool: &RpcPool, count: usize) -> Result<Vec<SendRequest>> {
  let payer = Keypair::new();
  let recipient = Keypair::new();
  let client = pool
    .get()
    .await
    .map_err(|err| anyhow!("rpc pool error: {err}"))?;

  let airdrop_sig = client
    .request_airdrop(&payer.pubkey(), LAMPORTS_PER_SOL)
    .await
    .context("airdrop failed (try a local validator or set TX_FILE)")?;

  let mut attempts = 0;
  let mut delay = Duration::from_millis(200);
  loop {
    attempts += 1;
    let confirmed = client
      .confirm_transaction_with_commitment(&airdrop_sig, CommitmentConfig::confirmed())
      .await
      .context("airdrop confirmation failed")?;
    if confirmed.value {
      break;
    }
    if attempts >= 15 {
      return Err(anyhow!("airdrop not confirmed after {attempts} attempts"));
    }
    sleep(delay).await;
    let next_ms = delay.as_millis().saturating_mul(2) as u64;
    delay = Duration::from_millis(next_ms.min(2_000));
  }

  let blockhash = client
    .get_latest_blockhash()
    .await
    .context("get_latest_blockhash failed")?;

  let mut requests = Vec::with_capacity(count);
  for idx in 0 .. count {
    let lamports = 1_000 + idx as u64;
    let ix = system_instruction::transfer(&payer.pubkey(), &recipient.pubkey(), lamports);
    let mut tx = Transaction::new_with_payer(&[ix], Some(&payer.pubkey()));
    tx.sign(&[&payer], blockhash);
    let versioned = VersionedTransaction::from(tx);
    let bytes = bincode::serialize(&versioned).context("bincode encode failed")?;
    let tx_b64 = general_purpose::STANDARD.encode(bytes);

    requests.push(SendRequest {
      request_id: format!("demo-{idx}"),
      tx_b64,
      priority: Priority::Normal,
    });
  }

  Ok(requests)
}

fn env_usize(key: &str, default: usize) -> usize {
  env::var(key)
    .ok()
    .and_then(|value| value.parse::<usize>().ok())
    .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
  env::var(key)
    .ok()
    .and_then(|value| value.parse::<u64>().ok())
    .unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
  env::var(key)
    .ok()
    .map(|value| value != "0")
    .unwrap_or(default)
}

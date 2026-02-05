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
use once_cell::sync::OnceCell;
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

const WAIT_BUCKETS_MS: [u64; 8] = [1, 5, 10, 50, 100, 500, 1_000, 5_000];
const WAIT_BUCKET_COUNT: usize = WAIT_BUCKETS_MS.len() + 1;

static SEND_QUEUE: OnceCell<Arc<dyn QueueInterface<SendTask> + Send + Sync>> = OnceCell::new();
static CONFIRM_QUEUE: OnceCell<Arc<dyn QueueInterface<ConfirmTask> + Send + Sync>> =
  OnceCell::new();
static SEND_METRICS: OnceCell<Arc<QueueMetrics>> = OnceCell::new();
static CONFIRM_METRICS: OnceCell<Arc<QueueMetrics>> = OnceCell::new();

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
  metrics_interval_ms: u64,
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
      metrics_interval_ms: env_u64("METRICS_INTERVAL_MS", 5_000),
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
  enqueued_at: TokioInstant,
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

#[async_trait]
trait QueueInterface<T>: Send + Sync {
  async fn push(&self, priority: Priority, item: T) -> Result<(), String>;
  async fn pop(&self) -> Option<T>;
}

struct PriorityQueueImpl<T> {
  inner: Mutex<PriorityQueue<QueueItem<T>, PriorityKey>>,
  notify: Notify,
  max_len: usize,
  seq: AtomicU64,
  metrics: Arc<QueueMetrics>,
}

struct QueueMetrics {
  name: &'static str,
  len: AtomicUsize,
  pushes: AtomicU64,
  pops: AtomicU64,
  drops: AtomicU64,
  wait_total_ms: AtomicU64,
  wait_max_ms: AtomicU64,
  wait_buckets: [AtomicUsize; WAIT_BUCKET_COUNT],
}

impl QueueMetrics {
  fn new(name: &'static str) -> Self {
    Self {
      name,
      len: AtomicUsize::new(0),
      pushes: AtomicU64::new(0),
      pops: AtomicU64::new(0),
      drops: AtomicU64::new(0),
      wait_total_ms: AtomicU64::new(0),
      wait_max_ms: AtomicU64::new(0),
      wait_buckets: std::array::from_fn(|_| AtomicUsize::new(0)),
    }
  }

  fn record_wait(&self, wait_ms: u64) {
    self.wait_total_ms.fetch_add(wait_ms, Ordering::Relaxed);
    let idx = wait_bucket_index(wait_ms);
    self.wait_buckets[idx].fetch_add(1, Ordering::Relaxed);

    let mut current = self.wait_max_ms.load(Ordering::Relaxed);
    while wait_ms > current {
      match self.wait_max_ms.compare_exchange(
        current,
        wait_ms,
        Ordering::Relaxed,
        Ordering::Relaxed,
      ) {
        Ok(_) => break,
        Err(next) => current = next,
      }
    }
  }

  fn snapshot(&self) -> QueueSnapshot {
    let buckets = std::array::from_fn(|idx| self.wait_buckets[idx].load(Ordering::Relaxed));
    QueueSnapshot {
      name: self.name,
      len: self.len.load(Ordering::Relaxed),
      pushes: self.pushes.load(Ordering::Relaxed),
      pops: self.pops.load(Ordering::Relaxed),
      drops: self.drops.load(Ordering::Relaxed),
      wait_total_ms: self.wait_total_ms.load(Ordering::Relaxed),
      wait_max_ms: self.wait_max_ms.load(Ordering::Relaxed),
      buckets,
    }
  }
}

struct QueueSnapshot {
  name: &'static str,
  len: usize,
  pushes: u64,
  pops: u64,
  drops: u64,
  wait_total_ms: u64,
  wait_max_ms: u64,
  buckets: [usize; WAIT_BUCKET_COUNT],
}

fn wait_bucket_index(wait_ms: u64) -> usize {
  for (idx, bound) in WAIT_BUCKETS_MS.iter().enumerate() {
    if wait_ms < *bound {
      return idx;
    }
  }
  WAIT_BUCKET_COUNT - 1
}

fn spawn_metrics_reporter(
  send_metrics: Arc<QueueMetrics>,
  confirm_metrics: Arc<QueueMetrics>,
  interval_ms: u64,
) {
  let interval_ms = interval_ms.max(500);
  tokio::spawn(async move {
    let mut ticker = interval(Duration::from_millis(interval_ms));
    loop {
      ticker.tick().await;
      report_queue_metrics(&send_metrics);
      report_queue_metrics(&confirm_metrics);
    }
  });
}

fn report_queue_metrics(metrics: &QueueMetrics) {
  let snapshot = metrics.snapshot();
  let avg_wait_ms = if snapshot.pops > 0 {
    snapshot.wait_total_ms / snapshot.pops
  } else {
    0
  };
  let buckets = snapshot.buckets;
  eprintln!(
    "queue_metrics name={} len={} pushes={} pops={} drops={} avg_wait_ms={} max_wait_ms={} \
     buckets=<1:{} <5:{} <10:{} <50:{} <100:{} <500:{} <1s:{} <5s:{} >=5s:{}",
    snapshot.name,
    snapshot.len,
    snapshot.pushes,
    snapshot.pops,
    snapshot.drops,
    avg_wait_ms,
    snapshot.wait_max_ms,
    buckets[0],
    buckets[1],
    buckets[2],
    buckets[3],
    buckets[4],
    buckets[5],
    buckets[6],
    buckets[7],
    buckets[8],
  );
}
impl<T> PriorityQueueImpl<T> {
  fn new(max_len: usize, metrics: Arc<QueueMetrics>) -> Self {
    Self {
      inner: Mutex::new(PriorityQueue::new()),
      notify: Notify::new(),
      max_len,
      seq: AtomicU64::new(0),
      metrics,
    }
  }

  async fn pop_once(&self) -> Option<T> {
    let mut guard = self.inner.lock().await;
    let popped = guard.pop();
    drop(guard);

    if let Some((item, _priority)) = popped {
      self.metrics.len.fetch_sub(1, Ordering::Relaxed);
      self.metrics.pops.fetch_add(1, Ordering::Relaxed);
      let wait_ms = TokioInstant::now()
        .saturating_duration_since(item.enqueued_at)
        .as_millis() as u64;
      self.metrics.record_wait(wait_ms);
      return Some(item.task);
    }

    None
  }
}

#[async_trait]
impl<T> QueueInterface<T> for PriorityQueueImpl<T>
where
  T: Send,
{
  async fn push(&self, priority: Priority, item: T) -> Result<(), String> {
    let prev = self.metrics.len.fetch_add(1, Ordering::Relaxed);
    if prev >= self.max_len {
      self.metrics.len.fetch_sub(1, Ordering::Relaxed);
      self.metrics.drops.fetch_add(1, Ordering::Relaxed);
      return Err("priority queue is full".to_string());
    }

    self.metrics.pushes.fetch_add(1, Ordering::Relaxed);
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
    guard.push(
      QueueItem {
        id,
        enqueued_at: TokioInstant::now(),
        task: item,
      },
      key,
    );
    drop(guard);
    self.notify.notify_one();
    Ok(())
  }

  async fn pop(&self) -> Option<T> {
    loop {
      if let Some(item) = self.pop_once().await {
        return Some(item);
      }
      self.notify.notified().await;
    }
  }
}

fn init_queues(
  cfg: &Config,
) -> (
  Arc<dyn QueueInterface<SendTask> + Send + Sync>,
  Arc<dyn QueueInterface<ConfirmTask> + Send + Sync>,
  Arc<QueueMetrics>,
  Arc<QueueMetrics>,
) {
  let max_len = cfg.queue_size.max(1);
  let send_metrics = SEND_METRICS
    .get_or_init(|| Arc::new(QueueMetrics::new("send")))
    .clone();
  let confirm_metrics = CONFIRM_METRICS
    .get_or_init(|| Arc::new(QueueMetrics::new("confirm")))
    .clone();
  let send_queue = SEND_QUEUE
    .get_or_init(|| {
      Arc::new(PriorityQueueImpl::<SendTask>::new(
        max_len,
        Arc::clone(&send_metrics),
      )) as Arc<dyn QueueInterface<SendTask> + Send + Sync>
    })
    .clone();
  let confirm_queue = CONFIRM_QUEUE
    .get_or_init(|| {
      Arc::new(PriorityQueueImpl::<ConfirmTask>::new(
        max_len,
        Arc::clone(&confirm_metrics),
      )) as Arc<dyn QueueInterface<ConfirmTask> + Send + Sync>
    })
    .clone();

  (send_queue, confirm_queue, send_metrics, confirm_metrics)
}

fn build_rpc_pool(cfg: &Config) -> Result<RpcPool> {
  let manager = RpcClientManager::new(cfg.rpc_url.clone(), CommitmentConfig::confirmed());
  Pool::builder(manager)
    .max_size(cfg.rpc_pool_size)
    .build()
    .map_err(|err| anyhow!("rpc pool build error: {err}"))
}

struct SendPool {
  queue: Arc<dyn QueueInterface<SendTask> + Send + Sync>,
}

impl SendPool {
  fn new(
    worker_count: usize,
    cfg: &Config,
    queue: Arc<dyn QueueInterface<SendTask> + Send + Sync>,
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
  queue: Arc<dyn QueueInterface<ConfirmTask> + Send + Sync>,
}

impl ConfirmPool {
  fn new(
    worker_count: usize,
    cfg: &Config,
    queue: Arc<dyn QueueInterface<ConfirmTask> + Send + Sync>,
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

  println!(
    "rpc_url={}, send_workers={}, confirm_workers={}, queue_size={}, rpc_pool_size={}",
    cfg.rpc_url, cfg.send_workers, cfg.confirm_workers, cfg.queue_size, cfg.rpc_pool_size
  );

  let (send_queue, confirm_queue, send_metrics, confirm_metrics) = init_queues(&cfg);
  let rpc_pool = build_rpc_pool(&cfg)?;
  let send_pool = Arc::new(SendPool::new(
    cfg.send_workers,
    &cfg,
    send_queue,
    rpc_pool.clone(),
  ));
  let confirm_pool = Arc::new(ConfirmPool::new(
    cfg.confirm_workers,
    &cfg,
    confirm_queue,
    rpc_pool.clone(),
  ));
  spawn_metrics_reporter(send_metrics, confirm_metrics, cfg.metrics_interval_ms);

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
  queue: Arc<dyn QueueInterface<SendTask> + Send + Sync>,
  in_flight: Arc<AtomicUsize>,
  idle_tick: Duration,
  trace: bool,
) where
  W: SendInterface + Send + Sync,
{
  let mut ticker = interval(idle_tick);
  loop {
    tokio::select! {
      task = queue.pop() => {
        let Some(task) = task else {
          continue;
        };
        in_flight.fetch_add(1, Ordering::Relaxed);
        let started = Instant::now();

        let result = worker.send_request(task.req, started).await;

        in_flight.fetch_sub(1, Ordering::Relaxed);
        let _ = task.resp.send(result);
      }
      _ = ticker.tick() => {
        if trace {
          eprintln!(
            "send idle worker_id={} in_flight={}",
            worker.id(),
            in_flight.load(Ordering::Relaxed)
          );
        }
      }
    }
  }
}

async fn confirm_worker_loop<W>(
  worker: W,
  queue: Arc<dyn QueueInterface<ConfirmTask> + Send + Sync>,
  in_flight: Arc<AtomicUsize>,
  idle_tick: Duration,
  trace: bool,
) where
  W: ConfirmInterface + Send + Sync,
{
  let mut ticker = interval(idle_tick);
  loop {
    tokio::select! {
      task = queue.pop() => {
        let Some(task) = task else {
          continue;
        };
        in_flight.fetch_add(1, Ordering::Relaxed);
        let started = Instant::now();

        let response = worker.confirm_request(task.req, started).await;

        in_flight.fetch_sub(1, Ordering::Relaxed);
        let _ = task.resp.send(response);
      }
      _ = ticker.tick() => {
        if trace {
          eprintln!(
            "confirm idle worker_id={} in_flight={}",
            worker.id(),
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

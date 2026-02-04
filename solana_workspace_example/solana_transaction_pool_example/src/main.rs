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

static SEND_QUEUE: OnceCell<Arc<dyn QueueInterface<SendTask> + Send + Sync>> = OnceCell::new();
static CONFIRM_QUEUE: OnceCell<Arc<dyn QueueInterface<ConfirmTask> + Send + Sync>> =
  OnceCell::new();

#[derive(Debug, Clone)]
struct Config {
  rpc_url: String,
  send_workers: usize,
  confirm_workers: usize,
  queue_size: usize,
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

    Self {
      rpc_url: env::var("RPC_URL").unwrap_or_else(|_| "http://localhost:8899".to_string()),
      send_workers: env_usize("SEND_WORKERS", 4),
      confirm_workers: env_usize("CONFIRM_WORKERS", 4),
      queue_size: env_usize("QUEUE_SIZE", 256),
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

#[derive(Debug, Clone)]
struct SendRequest {
  request_id: String,
  tx_b64: String,
}

#[derive(Debug)]
struct SendResponse {
  request_id: String,
  signature: String,
  worker_id: usize,
  elapsed_ms: u128,
}

struct SendTask {
  req: SendRequest,
  resp: oneshot::Sender<Result<SendResponse, String>>,
}

#[derive(Debug)]
struct ConfirmRequest {
  request_id: String,
  signature: Signature,
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
  client: RpcClient,
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
    let send_fut = self.client.send_transaction(&tx);
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
  client: RpcClient,
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
        };
      }

      attempts += 1;
      let remaining = deadline.saturating_duration_since(TokioInstant::now());
      let call_timeout = self.rpc_timeout.min(remaining);

      let status_result = timeout(
        call_timeout,
        self
          .client
          .get_signature_status_with_commitment(&req.signature, self.commitment),
      )
      .await;

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

#[async_trait]
trait QueueInterface<T>: Send + Sync {
  async fn push(&self, priority: Priority, item: T) -> Result<(), String>;
  async fn pop(&self) -> Option<T>;
}

struct PriorityQueueImpl<T> {
  inner: Mutex<PriorityQueue<QueueItem<T>, PriorityKey>>,
  notify: Notify,
  max_len: usize,
  len: AtomicUsize,
  seq: AtomicU64,
}

impl<T> PriorityQueueImpl<T> {
  fn new(max_len: usize) -> Self {
    Self {
      inner: Mutex::new(PriorityQueue::new()),
      notify: Notify::new(),
      max_len,
      len: AtomicUsize::new(0),
      seq: AtomicU64::new(0),
    }
  }

  async fn pop_once(&self) -> Option<T> {
    let mut guard = self.inner.lock().await;
    let popped = guard.pop();
    drop(guard);

    if let Some((item, _priority)) = popped {
      self.len.fetch_sub(1, Ordering::Relaxed);
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
    let prev = self.len.fetch_add(1, Ordering::Relaxed);
    if prev >= self.max_len {
      self.len.fetch_sub(1, Ordering::Relaxed);
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
) {
  let max_len = cfg.queue_size.max(1);
  let send_queue = SEND_QUEUE
    .get_or_init(|| {
      Arc::new(PriorityQueueImpl::<SendTask>::new(max_len))
        as Arc<dyn QueueInterface<SendTask> + Send + Sync>
    })
    .clone();
  let confirm_queue = CONFIRM_QUEUE
    .get_or_init(|| {
      Arc::new(PriorityQueueImpl::<ConfirmTask>::new(max_len))
        as Arc<dyn QueueInterface<ConfirmTask> + Send + Sync>
    })
    .clone();

  (send_queue, confirm_queue)
}

struct SendPool {
  queue: Arc<dyn QueueInterface<SendTask> + Send + Sync>,
}

impl SendPool {
  fn new(
    rpc_url: &str,
    worker_count: usize,
    cfg: &Config,
    queue: Arc<dyn QueueInterface<SendTask> + Send + Sync>,
  ) -> Self {
    for id in 0 .. worker_count {
      let in_flight = Arc::new(AtomicUsize::new(0));
      let worker = SendWorker {
        id,
        client: RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed()),
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
    let task = SendTask { req, resp: resp_tx };

    self
      .queue
      .push(Priority::Normal, task)
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
    rpc_url: &str,
    worker_count: usize,
    cfg: &Config,
    queue: Arc<dyn QueueInterface<ConfirmTask> + Send + Sync>,
  ) -> Self {
    for id in 0 .. worker_count {
      let in_flight = Arc::new(AtomicUsize::new(0));
      let worker = ConfirmWorker {
        id,
        client: RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed()),
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
    let task = ConfirmTask { req, resp: resp_tx };

    self
      .queue
      .push(Priority::Normal, task)
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
  let rpc_url = cfg.rpc_url.clone();

  println!(
    "rpc_url={}, send_workers={}, confirm_workers={}, queue_size={}",
    cfg.rpc_url, cfg.send_workers, cfg.confirm_workers, cfg.queue_size
  );

  let (send_queue, confirm_queue) = init_queues(&cfg);
  let send_pool = Arc::new(SendPool::new(&rpc_url, cfg.send_workers, &cfg, send_queue));
  let confirm_pool = Arc::new(ConfirmPool::new(
    &rpc_url,
    cfg.confirm_workers,
    &cfg,
    confirm_queue,
  ));

  let seed_client =
    RpcClient::new_with_commitment(cfg.rpc_url.clone(), CommitmentConfig::confirmed());
  let requests = if let Some(path) = &cfg.tx_file {
    load_requests_from_file(path)?
  } else {
    build_demo_requests(&seed_client, cfg.demo_count).await?
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
            "sent request_id={}, signature={}, worker={}, elapsed_ms={}",
            sent.request_id, sent.signature, sent.worker_id, sent.elapsed_ms
          );
          let signature = Signature::from_str(&sent.signature)
            .map_err(|e| anyhow!("invalid signature {}: {e}", sent.signature))?;
          let confirm_req = ConfirmRequest {
            request_id: sent.request_id.clone(),
            signature,
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
            "confirmed request_id={}, signature={}, worker={}, attempts={}, elapsed_ms={}",
            confirm.request_id,
            confirm.signature,
            confirm.worker_id,
            confirm.attempts,
            confirm.elapsed_ms
          );
        } else {
          println!(
            "failed request_id={}, signature={}, worker={}, attempts={}, elapsed_ms={}, reason={}",
            confirm.request_id,
            confirm.signature,
            confirm.worker_id,
            confirm.attempts,
            confirm.elapsed_ms,
            confirm.reason.unwrap_or_else(|| "unknown".to_string())
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
    let mut parts = line.splitn(2, ',');
    let request_id = parts
      .next()
      .map(str::trim)
      .filter(|s| !s.is_empty())
      .ok_or_else(|| anyhow!("line {} missing request_id", idx + 1))?;
    let tx_b64 = parts
      .next()
      .map(str::trim)
      .filter(|s| !s.is_empty())
      .ok_or_else(|| anyhow!("line {} missing base64 tx", idx + 1))?;

    requests.push(SendRequest {
      request_id: request_id.to_string(),
      tx_b64: tx_b64.to_string(),
    });
  }

  if requests.is_empty() {
    return Err(anyhow!("TX_FILE has no valid requests"));
  }

  Ok(requests)
}

async fn build_demo_requests(client: &RpcClient, count: usize) -> Result<Vec<SendRequest>> {
  let payer = Keypair::new();
  let recipient = Keypair::new();

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

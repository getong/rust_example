use std::{
  sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
  },
  time::{Duration, Instant},
};

use async_trait::async_trait;
use solana_commitment_config::CommitmentConfig;
use tokio::time::{interval, sleep, sleep_until, timeout, Instant as TokioInstant};

use crate::{
  config::Config,
  queue::ShardedQueue,
  requests::{
    decode_transaction, ConfirmRequest, ConfirmResponse, ConfirmTask, SendRequest, SendResponse,
    SendTask,
  },
  rpc_pool::{drop_rate_limited, is_rate_limited, EndpointSelector, RpcPool},
};

#[async_trait]
trait SendInterface {
  fn id(&self) -> usize;
  async fn send_request(&self, req: SendRequest, started: Instant) -> Result<SendResponse, String>;
}

pub struct SendWorker {
  id: usize,
  pool: RpcPool,
  endpoints: Arc<EndpointSelector>,
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
      Ok(Err(err)) => {
        let err_str = err.to_string();
        if is_rate_limited(&err_str) {
          drop_rate_limited(
            client,
            &self.pool,
            &self.endpoints,
            self.trace,
            self.id,
            "send",
          );
        }
        return Err(format!("rpc send error: {err_str}"));
      }
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

pub struct ConfirmWorker {
  id: usize,
  pool: RpcPool,
  endpoints: Arc<EndpointSelector>,
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

      let status_result = timeout(
        call_timeout,
        client.get_signature_status_with_commitment(&req.signature, self.commitment),
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
            priority: req.priority,
          };
        }
        Ok(Ok(Some(Err(err)))) => {
          let err_str = err.to_string();
          if is_rate_limited(&err_str) {
            drop_rate_limited(
              client,
              &self.pool,
              &self.endpoints,
              self.trace,
              self.id,
              "confirm",
            );
          }
          return ConfirmResponse {
            request_id: req.request_id,
            signature: req.signature.to_string(),
            confirmed: false,
            reason: Some(format!("transaction error: {err_str}")),
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

pub struct SendPool {
  queue: Arc<ShardedQueue<SendTask>>,
}

impl SendPool {
  pub fn new(
    worker_count: usize,
    cfg: &Config,
    queue: Arc<ShardedQueue<SendTask>>,
    rpc_pool: RpcPool,
    endpoints: Arc<EndpointSelector>,
  ) -> Self {
    for id in 0 .. worker_count {
      let in_flight = Arc::new(AtomicUsize::new(0));
      let worker = SendWorker {
        id,
        pool: rpc_pool.clone(),
        endpoints: endpoints.clone(),
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

  pub async fn submit(&self, req: SendRequest) -> Result<SendResponse, String> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
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

pub struct ConfirmPool {
  queue: Arc<ShardedQueue<ConfirmTask>>,
}

impl ConfirmPool {
  pub fn new(
    worker_count: usize,
    cfg: &Config,
    queue: Arc<ShardedQueue<ConfirmTask>>,
    rpc_pool: RpcPool,
    endpoints: Arc<EndpointSelector>,
  ) -> Self {
    for id in 0 .. worker_count {
      let in_flight = Arc::new(AtomicUsize::new(0));
      let worker = ConfirmWorker {
        id,
        pool: rpc_pool.clone(),
        endpoints: endpoints.clone(),
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

  pub async fn submit(&self, req: ConfirmRequest) -> Result<ConfirmResponse, String> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
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

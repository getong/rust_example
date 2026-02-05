use std::sync::{
  atomic::{AtomicUsize, Ordering},
  Arc,
};

use anyhow::{anyhow, Result};
use deadpool::managed::{Manager, Metrics, Object, Pool, RecycleResult};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;

use crate::config::Config;

pub struct EndpointSelector {
  urls: Vec<String>,
  index: AtomicUsize,
}

impl EndpointSelector {
  pub fn new(urls: Vec<String>) -> Self {
    Self {
      urls,
      index: AtomicUsize::new(0),
    }
  }

  pub fn list(&self) -> &[String] {
    &self.urls
  }

  fn current(&self) -> String {
    let idx = self.index.load(Ordering::Relaxed) % self.urls.len();
    self.urls[idx].clone()
  }

  pub fn rotate(&self) -> String {
    let idx = self.index.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
    self.urls[idx % self.urls.len()].clone()
  }
}

#[derive(Clone)]
pub struct RpcClientManager {
  endpoints: Arc<EndpointSelector>,
  commitment: CommitmentConfig,
}

impl RpcClientManager {
  pub fn new(endpoints: Arc<EndpointSelector>, commitment: CommitmentConfig) -> Self {
    Self {
      endpoints,
      commitment,
    }
  }
}

impl Manager for RpcClientManager {
  type Type = RpcClient;
  type Error = anyhow::Error;

  fn create(&self) -> impl std::future::Future<Output = Result<Self::Type, Self::Error>> + Send {
    let rpc_url = self.endpoints.current();
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

pub type RpcPool = Pool<RpcClientManager>;

pub fn build_rpc_pool(cfg: &Config, endpoints: Arc<EndpointSelector>) -> Result<RpcPool> {
  let manager = RpcClientManager::new(endpoints, CommitmentConfig::confirmed());
  Pool::builder(manager)
    .max_size(cfg.rpc_pool_size)
    .build()
    .map_err(|err| anyhow!("rpc pool build error: {err}"))
}

pub fn is_rate_limited(err: &str) -> bool {
  let lower = err.to_lowercase();
  lower.contains("rate limit")
    || lower.contains("ratelimit")
    || lower.contains("too many requests")
    || lower.contains("429")
}

pub fn handle_rate_limit(
  pool: &RpcPool,
  endpoints: &EndpointSelector,
  trace: bool,
  worker_id: usize,
  context: &str,
) {
  let next = endpoints.rotate();
  pool.retain(|_, _| false);
  if trace {
    eprintln!(
      "rate limit context={} worker_id={} switch_endpoint={}",
      context, worker_id, next
    );
  }
}

pub fn drop_rate_limited(
  client: Object<RpcClientManager>,
  pool: &RpcPool,
  endpoints: &EndpointSelector,
  trace: bool,
  worker_id: usize,
  context: &str,
) {
  let _ = Object::take(client);
  handle_rate_limit(pool, endpoints, trace, worker_id, context);
}

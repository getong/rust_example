use std::env;

#[derive(Debug, Clone)]
pub struct Config {
  pub rpc_urls: Vec<String>,
  pub send_workers: usize,
  pub confirm_workers: usize,
  pub queue_size: usize,
  pub rpc_pool_size: usize,
  pub tx_file: Option<String>,
  pub demo_count: usize,
  pub send_timeout_ms: u64,
  pub confirm_timeout_ms: u64,
  pub confirm_rpc_timeout_ms: u64,
  pub confirm_max_retries: usize,
  pub confirm_initial_delay_ms: u64,
  pub confirm_max_delay_ms: u64,
  pub worker_idle_tick_ms: u64,
  pub trace_workers: bool,
}

impl Config {
  pub fn from_env() -> Self {
    let idle_tick_ms = env_u64("WORKER_IDLE_TICK_MS", 5_000);
    let worker_idle_tick_ms = if idle_tick_ms == 0 {
      5_000
    } else {
      idle_tick_ms
    };
    let send_workers = env_usize("SEND_WORKERS", 4);
    let confirm_workers = env_usize("CONFIRM_WORKERS", 4);
    let rpc_urls = parse_rpc_urls();
    let rpc_pool_size = env_usize(
      "RPC_POOL_SIZE",
      send_workers.saturating_add(confirm_workers),
    )
    .max(1);

    Self {
      rpc_urls,
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
      confirm_max_delay_ms: env_u64("CONFIRM_MAX_DELAY_MS", 4_000),
      worker_idle_tick_ms,
      trace_workers: env_bool("TRACE_WORKER", false),
    }
  }
}

fn parse_rpc_urls() -> Vec<String> {
  if let Ok(value) = env::var("RPC_URLS") {
    let urls: Vec<String> = value
      .split(|ch: char| ch == ',' || ch.is_whitespace())
      .map(str::trim)
      .filter(|item| !item.is_empty())
      .map(|item| item.to_string())
      .collect();
    if !urls.is_empty() {
      return urls;
    }
  }

  vec![env::var("RPC_URL").unwrap_or_else(|_| "http://localhost:8899".to_string())]
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

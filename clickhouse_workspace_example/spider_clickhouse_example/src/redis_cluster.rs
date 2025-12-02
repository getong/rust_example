use std::{sync::Arc, time::Duration};

use once_cell::sync::Lazy;
use redis::{AsyncCommands, cluster::ClusterClient, cluster_async::ClusterConnection};
use tokio::{
  sync::{Mutex, OnceCell},
  time::timeout,
};

#[derive(Clone)]
pub struct RedisSettings {
  pub nodes: String,
  pub timeout_ms: u64,
}

pub type SharedRedis = Arc<Mutex<ClusterConnection>>;

pub static REDIS_CONN: Lazy<OnceCell<SharedRedis>> = Lazy::new(|| OnceCell::const_new());

pub async fn redis_connection(
  settings: &RedisSettings,
) -> Result<SharedRedis, Box<dyn std::error::Error>> {
  let cfg = settings.clone();
  REDIS_CONN
    .get_or_try_init(|| async move {
      let conn = timeout(
        Duration::from_millis(cfg.timeout_ms),
        connect_redis_cluster(&cfg.nodes),
      )
      .await
      .map_err(|_| {
        format!(
          "Redis connect timed out after {}ms (nodes={})",
          cfg.timeout_ms, cfg.nodes
        )
      })??;
      Ok(Arc::new(Mutex::new(conn)))
    })
    .await
    .map(Arc::clone)
}

pub async fn set_string(
  conn: &SharedRedis,
  key: &str,
  value: impl Into<String>,
) -> Result<(), Box<dyn std::error::Error>> {
  let mut guard = conn.lock().await;
  let _: () = guard.set(key, value.into()).await?;
  Ok(())
}

async fn connect_redis_cluster(
  urls: &str,
) -> Result<ClusterConnection, Box<dyn std::error::Error>> {
  let nodes = urls
    .split(',')
    .filter_map(|s| {
      let trimmed = s.trim();
      if trimmed.is_empty() {
        None
      } else if trimmed.starts_with("redis://") {
        Some(trimmed.to_string())
      } else {
        Some(format!("redis://{trimmed}"))
      }
    })
    .collect::<Vec<_>>();

  if nodes.is_empty() {
    return Err("at least one Redis node URL is required".into());
  }

  let client = ClusterClient::new(nodes)?;
  let mut conn = client.get_async_connection().await?;
  // Health check to fail fast if the cluster is unreachable.
  let _: String = redis::cmd("PING").query_async(&mut conn).await?;
  Ok(conn)
}

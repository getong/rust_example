use std::{path::Path, time::Duration};

use anyhow::Context;
use openraft::async_runtime::WatchReceiver;
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::Serialize;
use sqlx::{FromRow, SqlitePool, sqlite::SqlitePoolOptions};

use crate::{
  GroupHandle, GroupId,
  proto::raft_kv::{
    DeleteValueRequest, RaftKvRequest, raft_kv_request::Op as KvRequestOp,
    raft_kv_response::Op as KvResponseOp,
  },
  store::ensure_linearizable_read,
  types_kv::Request as KvWriteRequest,
};

pub const PENDING_KEY_PREFIX: &str = "__sqlite_pending__:";

#[derive(Debug, Clone)]
pub struct SqliteCache {
  sqlite: SqlitePool,
  redis: ConnectionManager,
  redis_key_prefix: String,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct CachedValue {
  pub key: String,
  pub value: String,
  pub updated_at: String,
}

impl SqliteCache {
  pub async fn connect(sqlite_url: &str, redis_url: &str) -> anyhow::Result<Self> {
    let sqlite = SqlitePoolOptions::new()
      .max_connections(5)
      .connect(sqlite_url)
      .await
      .with_context(|| format!("connect sqlite: {sqlite_url}"))?;

    let redis_client =
      redis::Client::open(redis_url).with_context(|| format!("open redis: {redis_url}"))?;
    let redis = ConnectionManager::new(redis_client)
      .await
      .with_context(|| format!("connect redis: {redis_url}"))?;

    let cache = Self {
      sqlite,
      redis,
      redis_key_prefix: "sqlite_cache:".to_string(),
    };
    cache.create_schema().await?;
    Ok(cache)
  }

  pub async fn connect_in_db_dir(db_dir: &Path, redis_url: &str) -> anyhow::Result<Self> {
    let sqlite_path = db_dir.join("cache.sqlite3");
    let sqlite_url = format!("sqlite://{}?mode=rwc", sqlite_path.display());
    Self::connect(&sqlite_url, redis_url).await
  }

  pub async fn write_redis(&self, key: &str, value: &str) -> anyhow::Result<()> {
    let mut redis = self.redis.clone();
    let _: () = redis
      .set(self.redis_key(key), value)
      .await
      .with_context(|| format!("write redis cache key={key}"))?;
    Ok(())
  }

  pub async fn read_cached(&self, key: &str) -> anyhow::Result<Option<String>> {
    let mut redis = self.redis.clone();
    let value: Option<String> = redis
      .get(self.redis_key(key))
      .await
      .with_context(|| format!("read redis cache key={key}"))?;
    if value.is_some() {
      return Ok(value);
    }

    self.read_sqlite_value(key).await
  }

  pub async fn read_redis_value(&self, key: &str) -> anyhow::Result<Option<String>> {
    let mut redis = self.redis.clone();
    redis
      .get(self.redis_key(key))
      .await
      .with_context(|| format!("read redis cache key={key}"))
  }

  pub async fn upsert_sqlite_value(&self, key: &str, value: &str) -> anyhow::Result<()> {
    sqlx::query(
      r#"
      INSERT INTO cached_values (key, value, updated_at)
      VALUES (?1, ?2, CURRENT_TIMESTAMP)
      ON CONFLICT(key) DO UPDATE SET
        value = excluded.value,
        updated_at = CURRENT_TIMESTAMP
      "#,
    )
    .bind(key)
    .bind(value)
    .execute(&self.sqlite)
    .await
    .with_context(|| format!("upsert sqlite cache key={key}"))?;
    Ok(())
  }

  pub async fn read_sqlite_value(&self, key: &str) -> anyhow::Result<Option<String>> {
    sqlx::query_scalar::<_, String>("SELECT value FROM cached_values WHERE key = ?1")
      .bind(key)
      .fetch_optional(&self.sqlite)
      .await
      .with_context(|| format!("read sqlite cache key={key}"))
  }

  pub async fn list_sqlite_values(&self) -> anyhow::Result<Vec<CachedValue>> {
    sqlx::query_as::<_, CachedValue>(
      r#"
      SELECT key, value, updated_at
      FROM cached_values
      ORDER BY key
      "#,
    )
    .fetch_all(&self.sqlite)
    .await
    .context("list sqlite cache values")
  }

  async fn create_schema(&self) -> anyhow::Result<()> {
    sqlx::query(
      r#"
      CREATE TABLE IF NOT EXISTS cached_values (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
      )
      "#,
    )
    .execute(&self.sqlite)
    .await
    .context("create sqlite cache schema")?;
    Ok(())
  }

  fn redis_key(&self, key: &str) -> String {
    format!("{}{}", self.redis_key_prefix, key)
  }
}

pub fn pending_key(key: &str) -> String {
  format!("{PENDING_KEY_PREFIX}{key}")
}

pub fn pending_data_key(openraft_key: &str) -> Option<&str> {
  openraft_key.strip_prefix(PENDING_KEY_PREFIX)
}

pub async fn run_sqlite_flush_worker(
  group_id: GroupId,
  group: GroupHandle,
  cache: SqliteCache,
  interval: Duration,
  mut shutdown_rx: crate::signal::ShutdownRx,
) {
  let mut tick = tokio::time::interval(interval);
  tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!("shutdown signal received, stopping sqlite cache flush worker");
        break;
      }
      _ = tick.tick() => {
        if let Err(err) = flush_once(&group_id, &group, &cache).await {
          tracing::warn!(group = %group_id, error = ?err, "sqlite cache flush failed");
        }
      }
    }
  }
}

async fn flush_once(
  group_id: &str,
  group: &GroupHandle,
  cache: &SqliteCache,
) -> anyhow::Result<()> {
  let metrics = group.raft.metrics().borrow_watched().clone();
  if !metrics.state.is_leader() {
    return Ok(());
  }

  ensure_linearizable_read(&group.raft)
    .await
    .map_err(|err| anyhow::anyhow!("{err:?}"))?;

  let entries = group.kv_data.entries().await?;
  for (openraft_key, _) in entries {
    let Some(data_key) = pending_data_key(&openraft_key) else {
      continue;
    };

    let Some(value) = cache.read_redis_value(data_key).await? else {
      tracing::warn!(key = data_key, "pending openraft key has no redis value");
      continue;
    };

    cache.upsert_sqlite_value(data_key, &value).await?;
    delete_pending_key(group_id, group, openraft_key).await?;
  }

  Ok(())
}

async fn delete_pending_key(
  group_id: &str,
  group: &GroupHandle,
  openraft_key: String,
) -> anyhow::Result<()> {
  let response = group
    .raft
    .client_write(KvWriteRequest::delete(openraft_key.clone()))
    .await
    .map_err(|err| anyhow::anyhow!("{err:?}"))?;

  if response.data.value.is_none() {
    tracing::debug!(group = group_id, key = %openraft_key, "deleted sqlite pending key");
  }

  Ok(())
}

pub fn pending_delete_request(group_id: String, openraft_key: String) -> RaftKvRequest {
  RaftKvRequest {
    group_id,
    op: Some(KvRequestOp::Delete(DeleteValueRequest {
      key: openraft_key,
    })),
  }
}

pub fn delete_succeeded(op: Option<KvResponseOp>) -> bool {
  matches!(op, Some(KvResponseOp::Delete(resp)) if resp.ok)
}

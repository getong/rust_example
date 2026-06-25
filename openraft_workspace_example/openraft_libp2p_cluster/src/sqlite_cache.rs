use std::{
  collections::{BTreeMap, hash_map::DefaultHasher},
  hash::{Hash, Hasher},
  path::Path,
  time::Duration,
};

use anyhow::Context;
use once_cell::sync::OnceCell;
use openraft::{ServerState, async_runtime::WatchReceiver};
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::Serialize;
use sqlx::{FromRow, SqlitePool, sqlite::SqlitePoolOptions};
use tarpc::{context, server::Serve};

use crate::{
  GroupHandle, GroupId, NodeId,
  network::{swarm::KvClient, transport::Libp2pNetworkFactory},
  openraft_group,
  proto::raft_kv::{
    DeleteValueRequest, ErrorResponse, RaftKvRequest, RaftKvResponse, SetValueRequest,
    raft_kv_request::Op as KvRequestOp, raft_kv_response::Op as KvResponseOp,
  },
  sqlite_sync_rpc::{
    SqliteFlushReport, SqliteFlushTask, SqliteSyncRpc, SqliteSyncRpcRequest,
    SqliteSyncRpcRequestMessage, SqliteSyncRpcResponse, SqliteSyncRpcResponseMessage,
  },
  store::ensure_linearizable_read,
  types_kv::Request as KvWriteRequest,
};

pub const PENDING_KEY_PREFIX: &str = "__sqlite_pending__:";
static SQLITE_CACHE: OnceCell<SqliteCache> = OnceCell::new();

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

pub fn set_sqlite_cache(cache: SqliteCache) -> Result<(), SqliteCache> {
  SQLITE_CACHE.set(cache)
}

pub fn sqlite_cache() -> Option<SqliteCache> {
  SQLITE_CACHE.get().cloned()
}

pub fn pending_key(key: &str) -> String {
  format!("{PENDING_KEY_PREFIX}{key}")
}

pub fn pending_data_key(openraft_key: &str) -> Option<&str> {
  openraft_key.strip_prefix(PENDING_KEY_PREFIX)
}

pub async fn run_sqlite_flush_worker(
  local_node_id: NodeId,
  group_id: GroupId,
  network: Libp2pNetworkFactory,
  kv_client: KvClient,
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
        if let Err(err) = flush_once(&local_node_id, &group_id, &network, &kv_client).await {
          tracing::warn!(group = %group_id, error = ?err, "sqlite cache flush failed");
        }
      }
    }
  }
}

async fn flush_once(
  local_node_id: &NodeId,
  group_id: &str,
  network: &Libp2pNetworkFactory,
  kv_client: &KvClient,
) -> anyhow::Result<()> {
  let Some(group) = openraft_group(group_id) else {
    tracing::debug!(
      group = group_id,
      "sqlite cache flush skipped: openraft group is not enabled on this node"
    );
    return Ok(());
  };

  let metrics = group.raft.metrics().borrow_watched().clone();
  if !metrics.state.is_leader() {
    return Ok(());
  }

  ensure_linearizable_read(&group.raft)
    .await
    .map_err(|err| anyhow::anyhow!("{err:?}"))?;

  let entries = group.kv_data.entries().await?;
  let tasks = pending_flush_tasks(entries);
  if tasks.is_empty() {
    return Ok(());
  }

  let follower_ids = sqlite_sync_executor_ids(local_node_id, &metrics);
  if follower_ids.is_empty() {
    tracing::warn!(
      group = group_id,
      pending = tasks.len(),
      "sqlite cache flush pending keys found but no openraft follower/learner is available"
    );
    return Ok(());
  }

  let assignments = assign_tasks_to_followers(tasks, &follower_ids);
  for (node_id, tasks) in assignments {
    let task_count = tasks.len();
    let request = sqlite_sync_request(group_id.to_string(), tasks);
    let response = match network.request_sqlite_sync(node_id.clone(), request).await {
      Ok(response) => response,
      Err(err) => {
        tracing::warn!(
          group = group_id,
          node_id = %node_id,
          tasks = task_count,
          error = ?err,
          "sqlite cache follower sync request failed"
        );
        continue;
      }
    };

    let report = sqlite_sync_report(response)?;
    handle_sqlite_flush_report(group_id, &group, kv_client, &node_id, report).await;
  }

  Ok(())
}

fn pending_flush_tasks(entries: Vec<(String, String)>) -> Vec<SqliteFlushTask> {
  let mut tasks = Vec::new();
  for (openraft_key, _) in entries {
    let Some(data_key) = pending_data_key(&openraft_key) else {
      continue;
    };

    let data_key = data_key.to_string();
    tasks.push(SqliteFlushTask {
      openraft_key,
      data_key,
    });
  }
  tasks
}

fn sqlite_sync_executor_ids(
  local_node_id: &NodeId,
  metrics: &crate::typ::RaftMetrics,
) -> Vec<NodeId> {
  let leader_id = metrics.current_leader.as_ref().unwrap_or(&metrics.id);
  let mut followers = metrics
    .membership_config
    .membership()
    .nodes()
    .filter_map(|(node_id, _)| {
      if node_id == leader_id || node_id == local_node_id {
        None
      } else {
        Some(node_id.clone())
      }
    })
    .collect::<Vec<_>>();
  followers.sort();
  followers
}

fn assign_tasks_to_followers(
  tasks: Vec<SqliteFlushTask>,
  followers: &[NodeId],
) -> BTreeMap<NodeId, Vec<SqliteFlushTask>> {
  let mut assignments = BTreeMap::new();
  if followers.is_empty() {
    return assignments;
  }

  for task in tasks {
    let mut hasher = DefaultHasher::new();
    task.data_key.hash(&mut hasher);
    let index = (hasher.finish() as usize) % followers.len();
    assignments
      .entry(followers[index].clone())
      .or_insert_with(Vec::new)
      .push(task);
  }

  assignments
}

fn sqlite_sync_request(
  group_id: GroupId,
  tasks: Vec<SqliteFlushTask>,
) -> SqliteSyncRpcRequestMessage {
  tarpc::ClientMessage::Request(tarpc::Request {
    context: context::current(),
    id: 0,
    message: SqliteSyncRpcRequest::FlushPending { group_id, tasks },
  })
}

fn sqlite_sync_report(response: SqliteSyncRpcResponseMessage) -> anyhow::Result<SqliteFlushReport> {
  match response.message {
    Ok(SqliteSyncRpcResponse::FlushPending(report)) => Ok(report),
    Err(err) => Err(anyhow::anyhow!("sqlite sync rpc failed: {err}")),
  }
}

async fn handle_sqlite_flush_report(
  group_id: &str,
  group: &GroupHandle,
  kv_client: &KvClient,
  node_id: &NodeId,
  report: SqliteFlushReport,
) {
  if let Some(error) = report.service_error {
    tracing::warn!(
      group = group_id,
      node_id = %node_id,
      error = %error,
      "sqlite cache follower sync service failed"
    );
    return;
  }

  for key in report.missing_redis_keys {
    tracing::warn!(
      group = group_id,
      node_id = %node_id,
      key = %key,
      "pending openraft key has no redis value on sqlite sync follower"
    );
  }

  for err in report.errors {
    tracing::warn!(
      group = group_id,
      node_id = %node_id,
      openraft_key = ?err.openraft_key,
      data_key = ?err.data_key,
      error = %err.message,
      "sqlite cache follower sync task failed"
    );
  }

  for openraft_key in report.synced_openraft_keys {
    if let Err(err) = delete_pending_key(group_id, group, kv_client, openraft_key.clone()).await {
      tracing::warn!(
        group = group_id,
        node_id = %node_id,
        key = %openraft_key,
        error = ?err,
        "delete sqlite pending openraft key failed"
      );
    }
  }
}

pub async fn record_pending_key(
  group_id: String,
  group: &GroupHandle,
  kv_client: &KvClient,
  data_key: &str,
) -> anyhow::Result<NodeId> {
  let openraft_key = pending_key(data_key);
  let (target_node_id, response) = submit_pending_key_request(
    group_id,
    group,
    kv_client,
    RaftKvRequest {
      group_id: String::new(),
      op: Some(KvRequestOp::Set(SetValueRequest {
        key: openraft_key.clone(),
        value: "1".to_string(),
      })),
    },
  )
  .await?;

  match response.op {
    Some(KvResponseOp::Set(resp)) if resp.ok => Ok(target_node_id),
    Some(KvResponseOp::Error(ErrorResponse { message })) => Err(anyhow::anyhow!(message)),
    other => Err(anyhow::anyhow!(
      "unexpected raft kv pending-key response: {other:?}"
    )),
  }
}

async fn delete_pending_key(
  group_id: &str,
  group: &GroupHandle,
  kv_client: &KvClient,
  openraft_key: String,
) -> anyhow::Result<()> {
  let (_, response) = submit_pending_key_request(
    group_id.to_string(),
    group,
    kv_client,
    pending_delete_request(group_id.to_string(), openraft_key.clone()),
  )
  .await?;

  match response.op {
    Some(KvResponseOp::Delete(resp)) if resp.ok => {
      tracing::debug!(group = group_id, key = %openraft_key, "deleted sqlite pending key");
      Ok(())
    }
    Some(KvResponseOp::Delete(_)) => {
      tracing::debug!(group = group_id, key = %openraft_key, "sqlite pending key already deleted");
      Ok(())
    }
    Some(KvResponseOp::Error(ErrorResponse { message })) => Err(anyhow::anyhow!(message)),
    other => Err(anyhow::anyhow!(
      "unexpected raft kv pending-key delete response: {other:?}"
    )),
  }
}

async fn submit_pending_key_request(
  group_id: String,
  group: &GroupHandle,
  kv_client: &KvClient,
  mut request: RaftKvRequest,
) -> anyhow::Result<(NodeId, RaftKvResponse)> {
  request.group_id = group_id.clone();

  let metrics = group.raft.metrics().borrow_watched().clone();
  let local_node_id = metrics.id.clone();
  if metrics.state.is_leader() {
    let response = submit_local_pending_key_request(&group_id, group, request).await?;
    return Ok((local_node_id, response));
  }

  let Some(leader_id) = metrics.current_leader else {
    anyhow::bail!("no leader available");
  };
  let Some(node) = metrics.membership_config.membership().get_node(&leader_id) else {
    anyhow::bail!("leader node not found in membership");
  };
  let (peer, addr) = crate::network::transport::parse_p2p_addr(&node.addr)
    .with_context(|| format!("parse leader address for node_id={leader_id}"))?;
  kv_client
    .connect(peer, addr)
    .await
    .map_err(|err| anyhow::anyhow!("connect to leader failed: {err}"))?;
  kv_client
    .request(peer, request)
    .await
    .map(|response| (leader_id, response))
    .map_err(|err| anyhow::anyhow!("forward pending key to leader failed: {err}"))
}

async fn submit_local_pending_key_request(
  group_id: &str,
  group: &GroupHandle,
  request: RaftKvRequest,
) -> anyhow::Result<RaftKvResponse> {
  let Some(op) = request.op else {
    anyhow::bail!("missing pending-key request op");
  };

  match op {
    KvRequestOp::Set(req) => group
      .raft
      .client_write(KvWriteRequest::Set {
        key: req.key.clone(),
        value: req.value.clone(),
      })
      .await
      .map(|resp| RaftKvResponse {
        op: Some(KvResponseOp::Set(crate::proto::raft_kv::SetValueResponse {
          ok: true,
          value: resp.data.value.unwrap_or(req.value),
        })),
      })
      .map_err(|err| anyhow::anyhow!("{err:?}")),
    KvRequestOp::Delete(req) => group
      .raft
      .client_write(KvWriteRequest::delete(req.key))
      .await
      .map(|_| RaftKvResponse {
        op: Some(KvResponseOp::Delete(
          crate::proto::raft_kv::DeleteValueResponse { ok: true },
        )),
      })
      .map_err(|err| anyhow::anyhow!("{err:?}")),
    _ => anyhow::bail!("unsupported pending-key request op for group_id={group_id}"),
  }
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

#[derive(Clone)]
pub struct SqliteSyncService;

impl SqliteSyncRpc for SqliteSyncService {
  async fn flush_pending(
    self,
    _: context::Context,
    group_id: GroupId,
    tasks: Vec<SqliteFlushTask>,
  ) -> SqliteFlushReport {
    execute_follower_flush(&group_id, tasks).await
  }
}

pub async fn process_sqlite_sync_rpc_request(
  request: SqliteSyncRpcRequestMessage,
) -> SqliteSyncRpcResponseMessage {
  match request {
    tarpc::ClientMessage::Request(request) => {
      let request_id = request.id;
      let response = SqliteSyncService
        .serve()
        .serve(request.context, request.message)
        .await;
      tarpc::Response {
        request_id,
        message: response,
      }
    }
    tarpc::ClientMessage::Cancel { request_id, .. } => tarpc::Response {
      request_id,
      message: Err(tarpc::ServerError::new(
        std::io::ErrorKind::Interrupted,
        "sqlite sync cancel messages are not processed by one-shot libp2p rpc".to_string(),
      )),
    },
    _ => tarpc::Response {
      request_id: 0,
      message: Err(tarpc::ServerError::new(
        std::io::ErrorKind::InvalidInput,
        "unsupported sqlite sync rpc message".to_string(),
      )),
    },
  }
}

async fn execute_follower_flush(group_id: &str, tasks: Vec<SqliteFlushTask>) -> SqliteFlushReport {
  let Some(cache) = sqlite_cache() else {
    return SqliteFlushReport::service_error("sqlite cache is disabled on this node");
  };

  let Some(group) = openraft_group(group_id) else {
    return SqliteFlushReport::service_error(format!(
      "openraft group {group_id} is not enabled on this node"
    ));
  };

  let metrics = group.raft.metrics().borrow_watched().clone();
  if metrics.state == ServerState::Leader {
    return SqliteFlushReport::service_error(
      "sqlite sync execution must run on an openraft follower or learner",
    );
  }

  if !matches!(metrics.state, ServerState::Follower | ServerState::Learner) {
    return SqliteFlushReport::service_error(format!(
      "openraft node is not ready for sqlite sync: state={:?}",
      metrics.state
    ));
  }

  let mut report = SqliteFlushReport::default();
  for task in tasks {
    match cache.read_redis_value(&task.data_key).await {
      Ok(Some(value)) => {
        if let Err(err) = cache.upsert_sqlite_value(&task.data_key, &value).await {
          report
            .errors
            .push(SqliteFlushReport::task_error(&task, err.to_string()));
          continue;
        }
        report.synced_openraft_keys.push(task.openraft_key);
      }
      Ok(None) => report.missing_redis_keys.push(task.data_key),
      Err(err) => report
        .errors
        .push(SqliteFlushReport::task_error(&task, err.to_string())),
    }
  }

  report
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn pending_flush_tasks_keeps_only_sqlite_pending_keys() {
    let tasks = pending_flush_tasks(vec![
      (pending_key("a"), "1".to_string()),
      ("normal".to_string(), "2".to_string()),
      (pending_key("b"), "3".to_string()),
    ]);

    let keys = tasks
      .into_iter()
      .map(|task| (task.openraft_key, task.data_key))
      .collect::<Vec<_>>();
    assert_eq!(
      keys,
      vec![
        (pending_key("a"), "a".to_string()),
        (pending_key("b"), "b".to_string()),
      ]
    );
  }

  #[test]
  fn assign_tasks_to_followers_is_stable_and_complete() {
    let followers = vec![NodeId::from("node-a"), NodeId::from("node-b")];
    let tasks = vec![
      SqliteFlushTask {
        openraft_key: pending_key("a"),
        data_key: "a".to_string(),
      },
      SqliteFlushTask {
        openraft_key: pending_key("b"),
        data_key: "b".to_string(),
      },
      SqliteFlushTask {
        openraft_key: pending_key("c"),
        data_key: "c".to_string(),
      },
    ];

    let first = assign_tasks_to_followers(tasks.clone(), &followers);
    let second = assign_tasks_to_followers(tasks, &followers);
    assert_eq!(
      first.keys().collect::<Vec<_>>(),
      second.keys().collect::<Vec<_>>()
    );

    let assigned = first.values().map(Vec::len).sum::<usize>();
    assert_eq!(assigned, 3);
  }

  #[test]
  fn assign_tasks_to_followers_handles_empty_followers() {
    let assignments = assign_tasks_to_followers(
      vec![SqliteFlushTask {
        openraft_key: pending_key("a"),
        data_key: "a".to_string(),
      }],
      &[],
    );

    assert!(assignments.is_empty());
  }
}

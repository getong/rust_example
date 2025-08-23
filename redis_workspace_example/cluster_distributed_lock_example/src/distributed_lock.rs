use std::{sync::Arc, time::Duration};

use anyhow::{Result, anyhow};
use redis::{
  AsyncCommands, RedisResult, Script, cluster::ClusterClient, cluster_async::ClusterConnection,
};
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct RedisDistributedLock {
  client: ClusterClient,
  connection: Arc<RwLock<ClusterConnection>>,
  lock_name: String,
  lock_value: String,
  ttl: Duration,
}

impl RedisDistributedLock {
  pub async fn new(nodes: Vec<String>, lock_name: String, ttl: Duration) -> Result<Self> {
    let client = ClusterClient::new(nodes)?;
    let connection = client.get_async_connection().await?;
    let lock_value = Uuid::new_v4().to_string();

    Ok(Self {
      client,
      connection: Arc::new(RwLock::new(connection)),
      lock_name,
      lock_value,
      ttl,
    })
  }

  async fn get_connection(&self) -> Result<tokio::sync::RwLockWriteGuard<'_, ClusterConnection>> {
    let mut con = self.connection.write().await;

    // Check if connection is still valid by sending a PING
    let ping_result: RedisResult<String> = redis::cmd("PING").query_async(&mut *con).await;

    if ping_result.is_err() {
      // Connection is broken, try to reconnect
      match self.client.get_async_connection().await {
        Ok(new_connection) => {
          *con = new_connection;
        }
        Err(e) => {
          return Err(anyhow!("Failed to reconnect: {}", e));
        }
      }
    }

    Ok(con)
  }

  pub async fn acquire(&self) -> Result<bool> {
    let mut con = self.get_connection().await?;
    let ttl_ms = self.ttl.as_millis() as u64;

    let result: RedisResult<Option<String>> = (*con)
      .set_options(
        &self.lock_name,
        &self.lock_value,
        redis::SetOptions::default()
          .conditional_set(redis::ExistenceCheck::NX)
          .with_expiration(redis::SetExpiry::PX(ttl_ms)),
      )
      .await;

    match result {
      Ok(Some(_)) => Ok(true),
      Ok(None) => Ok(false),
      Err(e) => Err(anyhow!("Failed to acquire lock: {}", e)),
    }
  }

  pub async fn acquire_with_retry(&self, max_retries: u32, retry_delay: Duration) -> Result<bool> {
    for attempt in 0 .. max_retries {
      if self.acquire().await? {
        println!("Lock acquired on attempt {}", attempt + 1);
        return Ok(true);
      }

      if attempt < max_retries - 1 {
        println!("Lock acquisition failed, retrying in {:?}...", retry_delay);
        tokio::time::sleep(retry_delay).await;
      }
    }

    Ok(false)
  }

  pub async fn release(&self) -> Result<bool> {
    let lua_script = r#"
            if redis.call("get", KEYS[1]) == ARGV[1] then
                return redis.call("del", KEYS[1])
            else
                return 0
            end
        "#;

    let mut con = self.get_connection().await?;
    let script = Script::new(lua_script);

    let result: RedisResult<i32> = script
      .key(&self.lock_name)
      .arg(&self.lock_value)
      .invoke_async(&mut *con)
      .await;

    match result {
      Ok(1) => Ok(true),
      Ok(0) => Ok(false),
      Ok(_) => Ok(false),
      Err(e) => Err(anyhow!("Failed to release lock: {}", e)),
    }
  }

  pub async fn extend(&self, additional_ttl: Duration) -> Result<bool> {
    let lua_script = r#"
            if redis.call("get", KEYS[1]) == ARGV[1] then
                return redis.call("pexpire", KEYS[1], ARGV[2])
            else
                return 0
            end
        "#;

    let mut con = self.get_connection().await?;
    let script = Script::new(lua_script);
    let ttl_ms = additional_ttl.as_millis() as u64;

    let result: RedisResult<i32> = script
      .key(&self.lock_name)
      .arg(&self.lock_value)
      .arg(ttl_ms)
      .invoke_async(&mut *con)
      .await;

    match result {
      Ok(1) => Ok(true),
      Ok(0) => Ok(false),
      Ok(_) => Ok(false),
      Err(e) => Err(anyhow!("Failed to extend lock: {}", e)),
    }
  }

  pub async fn is_locked(&self) -> Result<bool> {
    let mut con = self.get_connection().await?;
    let result: RedisResult<bool> = (*con).exists(&self.lock_name).await;

    match result {
      Ok(exists) => Ok(exists),
      Err(e) => Err(anyhow!("Failed to check lock status: {}", e)),
    }
  }

  pub async fn get_lock_info(&self) -> Result<Option<LockInfo>> {
    let mut con = self.get_connection().await?;

    let value: RedisResult<Option<String>> = (*con).get(&self.lock_name).await;
    let ttl: RedisResult<isize> = (*con).pttl(&self.lock_name).await;

    match (value, ttl) {
      (Ok(Some(v)), Ok(ttl_ms)) if ttl_ms > 0 => Ok(Some(LockInfo {
        lock_name: self.lock_name.clone(),
        lock_value: v.clone(),
        ttl_ms: ttl_ms as u64,
        is_owned_by_me: v == self.lock_value,
      })),
      _ => Ok(None),
    }
  }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LockInfo {
  pub lock_name: String,
  pub lock_value: String,
  pub ttl_ms: u64,
  pub is_owned_by_me: bool,
}

impl LockInfo {
  pub fn remaining_time(&self) -> Duration {
    Duration::from_millis(self.ttl_ms)
  }
}

pub struct LockGuard {
  lock: RedisDistributedLock,
  auto_release: bool,
}

impl LockGuard {
  pub fn new(lock: RedisDistributedLock, auto_release: bool) -> Self {
    Self { lock, auto_release }
  }

  #[allow(dead_code)]
  pub async fn release(self) -> Result<bool> {
    self.lock.release().await
  }
}

impl Drop for LockGuard {
  fn drop(&mut self) {
    if self.auto_release {
      let lock = self.lock.clone();
      tokio::spawn(async move {
        if let Err(e) = lock.release().await {
          eprintln!("Failed to auto-release lock: {}", e);
        }
      });
    }
  }
}

impl Clone for RedisDistributedLock {
  fn clone(&self) -> Self {
    Self {
      client: self.client.clone(),
      connection: Arc::clone(&self.connection),
      lock_name: self.lock_name.clone(),
      lock_value: self.lock_value.clone(),
      ttl: self.ttl,
    }
  }
}

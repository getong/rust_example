use std::{sync::Arc, time::Duration};

use redis::{AsyncCommands, RedisResult, Script, aio::ConnectionManager};
use thiserror::Error;
use tokio::{sync::Mutex, task::JoinHandle};

#[derive(Debug, Error)]
pub enum LockError {
  #[error("failed to acquire lock within timeout")]
  AcquireTimeout,
  #[error("redis error: {0}")]
  Redis(#[from] redis::RedisError),
  #[error("watchdog already running")]
  WatchdogAlreadyRunning,
  #[error("lock not held")]
  NotHeld,
}

#[derive(Clone)]
pub struct RedisLock {
  conn: ConnectionManager,
  key: String,
  token: Arc<str>,
  ttl_ms: u64,
  inner: Arc<Mutex<WatchdogState>>,
}

struct WatchdogState {
  handle: Option<JoinHandle<()>>,
  stop: bool,
}

impl RedisLock {
  pub async fn new(
    redis_url: &str,
    key: impl Into<String>,
    ttl_ms: u64,
  ) -> Result<Self, LockError> {
    let client = redis::Client::open(redis_url)?;
    let manager = ConnectionManager::new(client).await?;
    let token = uuid::Uuid::new_v4().to_string();
    Ok(Self {
      conn: manager,
      key: key.into(),
      token: token.into(),
      ttl_ms,
      inner: Arc::new(Mutex::new(WatchdogState {
        handle: None,
        stop: false,
      })),
    })
  }

  pub async fn acquire_with_retry(
    &self,
    acquire_timeout: Duration,
    retry_interval: Duration,
  ) -> Result<(), LockError> {
    let start = tokio::time::Instant::now();
    let mut conn = self.conn.clone();

    let set_script = Script::new(
      r#"
            return redis.call('SET', KEYS[1], ARGV[1], 'NX', 'PX', ARGV[2]) and 1 or 0
            "#,
    );

    while start.elapsed() < acquire_timeout {
      let ok: i64 = set_script
        .key(&self.key)
        .arg(&*self.token)
        .arg(self.ttl_ms)
        .invoke_async(&mut conn)
        .await?;

      if ok == 1 {
        return Ok(());
      }

      let jitter = rand::random::<u64>() % (retry_interval.as_millis() as u64 / 2 + 1);
      tokio::time::sleep(retry_interval + Duration::from_millis(jitter)).await;
    }

    Err(LockError::AcquireTimeout)
  }

  pub async fn unlock(&self) -> Result<(), LockError> {
    let mut conn = self.conn.clone();
    let script = Script::new(
      r#"
            if redis.call('GET', KEYS[1]) == ARGV[1] then
              return redis.call('DEL', KEYS[1])
            else
              return 0
            end
            "#,
    );
    let deleted: i64 = script
      .key(&self.key)
      .arg(&*self.token)
      .invoke_async(&mut conn)
      .await?;
    if deleted == 1 {
      Ok(())
    } else {
      Err(LockError::NotHeld)
    }
  }

  pub async fn refresh(&self) -> Result<(), LockError> {
    let mut conn = self.conn.clone();
    let script = Script::new(
      r#"
            if redis.call('GET', KEYS[1]) == ARGV[1] then
              return redis.call('PEXPIRE', KEYS[1], ARGV[2])
            else
              return 0
            end
            "#,
    );
    let ok: i64 = script
      .key(&self.key)
      .arg(&*self.token)
      .arg(self.ttl_ms)
      .invoke_async(&mut conn)
      .await?;
    if ok == 1 {
      Ok(())
    } else {
      Err(LockError::NotHeld)
    }
  }

  pub async fn start_watchdog(&self, interval: Duration) -> Result<(), LockError> {
    let mut guard = self.inner.lock().await;
    if guard.handle.is_some() {
      return Err(LockError::WatchdogAlreadyRunning);
    }
    guard.stop = false;
    let key = self.key.clone();
    let token = self.token.clone();
    let ttl_ms = self.ttl_ms;
    let conn = self.conn.clone();
    let stop_flag = self.inner.clone();

    let handle = tokio::spawn(async move {
      let mut conn = conn;
      let refresh_script = Script::new(
        r#"
                if redis.call('GET', KEYS[1]) == ARGV[1] then
                  return redis.call('PEXPIRE', KEYS[1], ARGV[2])
                else
                  return 0
                end
                "#,
      );

      loop {
        if stop_flag.lock().await.stop {
          break;
        }
        match refresh_script
          .key(&key)
          .arg(&*token)
          .arg(ttl_ms)
          .invoke_async(&mut conn)
          .await
        {
          Ok(1) => {}
          Ok(0) => {
            break;
          }
          Err(_) => {}
          _ => {}
        }
        tokio::time::sleep(interval).await;
      }
    });

    guard.handle = Some(handle);
    Ok(())
  }

  pub async fn stop_watchdog(&self) {
    let mut guard = self.inner.lock().await;
    if let Some(handle) = guard.handle.take() {
      guard.stop = true;

      drop(guard);
      let _ = handle.await;
      let mut g = self.inner.lock().await;
      g.stop = false;
    }
  }

  pub async fn is_held(&self) -> RedisResult<bool> {
    let mut conn = self.conn.clone();
    let v: Option<String> = conn.get(&self.key).await?;
    Ok(matches!(v, Some(s) if s == *self.token))
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let url = "redis://127.0.0.1/";
  let lock = RedisLock::new(url, "locks:demo_job", 8000).await?; // TTL 8s

  lock
    .acquire_with_retry(Duration::from_secs(3), Duration::from_millis(200))
    .await?;
  println!("acquired! token={}", &*lock.token);

  lock.start_watchdog(Duration::from_secs(3)).await?;

  for i in 1 ..= 10 {
    println!("working... {i}");
    tokio::time::sleep(Duration::from_secs(1)).await;
  }

  lock.stop_watchdog().await;
  lock.unlock().await?;
  println!("released.");
  Ok(())
}

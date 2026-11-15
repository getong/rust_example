use std::{env, io, time::SystemTime};

use redis::{
  AsyncCommands, Script,
  cluster::ClusterClient,
  cluster_async::ClusterConnection,
  streams::{
    StreamAutoClaimOptions, StreamAutoClaimReply, StreamId, StreamReadOptions, StreamReadReply,
  },
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::time::{Duration, sleep};

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

const OUTBOX_KEY: &str = "{users}:outbox";
const CONSUMER_GROUP: &str = "postgres-writers";
const USER_CACHE_TTL_SECS: u64 = 300;
const CLAIM_IDLE_MS: u64 = 30_000;
const BATCH_SIZE: usize = 32;
const ENQUEUE_USER: &str = r#"
local event_id = redis.call('XADD', KEYS[1], '*', 'user', ARGV[1])
redis.call('SET', KEYS[2], ARGV[1], 'EX', ARGV[2])
return event_id
"#;
const ACKNOWLEDGE_EVENT: &str = r#"
redis.call('XACK', KEYS[1], ARGV[1], ARGV[2])
return redis.call('XDEL', KEYS[1], ARGV[2])
"#;

#[derive(Serialize, Deserialize, Debug)]
struct User {
  id: i32,
  name: String,
  email: String,
}

struct Config {
  database_url: String,
  redis_nodes: Vec<String>,
  consumer_name: String,
  write_demo_user: bool,
}

impl Config {
  fn from_env() -> AppResult<Self> {
    let database_url = env::var("DATABASE_URL")?;
    let redis_nodes: Vec<String> = env::var("REDIS_CLUSTER_NODES")?
      .split(',')
      .map(str::trim)
      .filter(|node| !node.is_empty())
      .map(str::to_owned)
      .collect();
    if redis_nodes.is_empty() {
      return Err(
        io::Error::other("REDIS_CLUSTER_NODES must contain at least one seed URL").into(),
      );
    }
    let consumer_name = format!(
      "{}-{}-{}",
      env::var("HOSTNAME").unwrap_or_else(|_| "worker".to_owned()),
      std::process::id(),
      SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_nanos()
    );
    let write_demo_user = env::var("WRITE_DEMO_USER")
      .unwrap_or_else(|_| "true".to_owned())
      .parse()?;
    Ok(Self {
      database_url,
      redis_nodes,
      consumer_name,
      write_demo_user,
    })
  }
}

#[tokio::main]
async fn main() -> AppResult<()> {
  dotenv::dotenv().ok();
  let config = Config::from_env()?;
  tokio::select! {
    result = run(config) => result,
    result = shutdown_signal() => {
      result?;
      println!("Stopping; unacknowledged outbox events remain available for recovery.");
      Ok(())
    }
  }
}

async fn run(config: Config) -> AppResult<()> {
  let pg_pool = PgPoolOptions::new()
    .max_connections(10)
    .acquire_timeout(Duration::from_secs(10))
    .test_before_acquire(true)
    .connect_lazy(&config.database_url)?;

  let redis_client = ClusterClient::builder(config.redis_nodes)
    .connection_timeout(Duration::from_secs(5))
    .response_timeout(Duration::from_secs(10))
    .retries(3)
    .build()?;
  let mut retry_delay = Duration::from_secs(1);
  loop {
    match sqlx::migrate!().run(&pg_pool).await {
      Ok(()) => break,
      Err(error) => {
        eprintln!("PostgreSQL initialization failed: {error}; retrying in {retry_delay:?}");
        sleep(retry_delay).await;
        retry_delay = (retry_delay * 2).min(Duration::from_secs(30));
      }
    }
  }

  println!("Redis Cluster → PostgreSQL sync is running. Press Ctrl+C to stop.");
  sync_in_background(
    &pg_pool,
    &redis_client,
    &config.consumer_name,
    config.write_demo_user,
  )
  .await
}

async fn ensure_consumer_group(redis_conn: &mut ClusterConnection) -> AppResult<()> {
  let result: redis::RedisResult<()> = redis::cmd("XGROUP")
    .arg("CREATE")
    .arg(OUTBOX_KEY)
    .arg(CONSUMER_GROUP)
    .arg("0-0")
    .arg("MKSTREAM")
    .query_async(redis_conn)
    .await;
  match result {
    Ok(()) => Ok(()),
    Err(error) if error.code() == Some("BUSYGROUP") => Ok(()),
    Err(error) => Err(error.into()),
  }
}

async fn write_user_to_cache(redis_conn: &mut ClusterConnection, user: &User) -> AppResult<String> {
  let user_json = serde_json::to_string(user)?;
  let event_id = Script::new(ENQUEUE_USER)
    .key(OUTBOX_KEY)
    .key(format!("{{users}}:cache:{}", user.id))
    .arg(user_json)
    .arg(USER_CACHE_TTL_SECS)
    .invoke_async(redis_conn)
    .await?;
  Ok(event_id)
}

async fn sync_in_background(
  pg_pool: &PgPool,
  redis_client: &ClusterClient,
  consumer_name: &str,
  mut demo_user_pending: bool,
) -> AppResult<()> {
  let mut retry_delay = Duration::from_secs(1);
  loop {
    let result: AppResult<()> = async {
      let mut redis_conn = redis_client.get_async_connection().await?;
      ensure_consumer_group(&mut redis_conn).await?;
      if demo_user_pending {
        let user = User {
          id: 1,
          name: "John Doe".to_owned(),
          email: "john.doe@example.com".to_owned(),
        };
        let event_id = write_user_to_cache(&mut redis_conn, &user).await?;
        demo_user_pending = false;
        println!("User cached and queued as event {event_id}.");
      }
      let mut claim_cursor = "0-0".to_owned();
      loop {
        let reclaimed: StreamAutoClaimReply = redis_conn
          .xautoclaim_options(
            OUTBOX_KEY,
            CONSUMER_GROUP,
            consumer_name,
            CLAIM_IDLE_MS,
            &claim_cursor,
            StreamAutoClaimOptions::default().count(BATCH_SIZE),
          )
          .await?;
        claim_cursor = reclaimed.next_stream_id;
        let mut had_failures = persist_events(pg_pool, &mut redis_conn, reclaimed.claimed).await;

        let options = StreamReadOptions::default()
          .group(CONSUMER_GROUP, consumer_name)
          .count(BATCH_SIZE)
          .block(2_000);
        let reply: StreamReadReply = redis_conn
          .xread_options(&[OUTBOX_KEY], &[">"], &options)
          .await?;
        for stream in reply.keys {
          had_failures |= persist_events(pg_pool, &mut redis_conn, stream.ids).await;
        }
        if had_failures {
          sleep(retry_delay).await;
          retry_delay = (retry_delay * 2).min(Duration::from_secs(30));
        } else {
          retry_delay = Duration::from_secs(1);
        }
      }
    }
    .await;
    if let Err(error) = result {
      eprintln!("Redis sync connection failed: {error}; retrying in {retry_delay:?}");
      sleep(retry_delay).await;
      retry_delay = (retry_delay * 2).min(Duration::from_secs(30));
    }
  }
}

async fn persist_events(
  pg_pool: &PgPool,
  redis_conn: &mut ClusterConnection,
  events: Vec<StreamId>,
) -> bool {
  let mut had_failures = false;
  for event in events {
    if let Err(error) = persist_event(pg_pool, redis_conn, &event).await {
      had_failures = true;
      eprintln!("Event {} left pending for retry: {error}", event.id);
    }
  }
  had_failures
}

async fn persist_event(
  pg_pool: &PgPool,
  redis_conn: &mut ClusterConnection,
  event: &StreamId,
) -> AppResult<()> {
  let user_json: String = event
    .get("user")
    .ok_or_else(|| io::Error::other("missing user payload"))?;
  let user: User = serde_json::from_str(&user_json)?;
  let (milliseconds, sequence) = event
    .id
    .split_once('-')
    .ok_or_else(|| io::Error::other("invalid Redis stream event ID"))?;
  let milliseconds: i64 = milliseconds.parse()?;
  let sequence: i64 = sequence.parse()?;

  sqlx::query(
    "INSERT INTO users (id, name, email, redis_event_ms, redis_event_seq)
     VALUES ($1, $2, $3, $4, $5)
     ON CONFLICT (id) DO UPDATE
     SET name = EXCLUDED.name, email = EXCLUDED.email,
         redis_event_ms = EXCLUDED.redis_event_ms, redis_event_seq = EXCLUDED.redis_event_seq
     WHERE (users.redis_event_ms, users.redis_event_seq)
         < (EXCLUDED.redis_event_ms, EXCLUDED.redis_event_seq)",
  )
  .bind(user.id)
  .bind(&user.name)
  .bind(&user.email)
  .bind(milliseconds)
  .bind(sequence)
  .execute(pg_pool)
  .await?;

  let _: i64 = Script::new(ACKNOWLEDGE_EVENT)
    .key(OUTBOX_KEY)
    .arg(CONSUMER_GROUP)
    .arg(&event.id)
    .invoke_async(redis_conn)
    .await?;
  println!(
    "Event {} persisted or already superseded; acknowledged.",
    event.id
  );
  Ok(())
}

async fn shutdown_signal() -> AppResult<()> {
  #[cfg(unix)]
  {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
      result = tokio::signal::ctrl_c() => result?,
      _ = terminate.recv() => {},
    }
  }
  #[cfg(not(unix))]
  tokio::signal::ctrl_c().await?;
  Ok(())
}

use std::env;

use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Pool, Postgres};
use tokio::time::{interval, Duration};

const USER_CACHE_TTL_SECS: i64 = 60 * 5;
const SYNC_INTERVAL_SECS: u64 = 5;

#[derive(Serialize, Deserialize, Debug)]
struct User {
  id: i32,
  name: String,
  email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Load environment variables
  dotenv::dotenv().ok();
  sqlx::any::install_default_drivers();

  let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
  let redis_url = env::var("REDIS_URL").expect("REDIS_URL must be set");

  // Create PostgreSQL connection pool
  let pg_pool = PgPool::connect(&database_url).await?;

  // Create Redis client and connection
  let redis_client = redis::Client::open(redis_url)?;
  let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;

  let sync_task = tokio::spawn(sync_redis_to_postgres_in_background(
    pg_pool.clone(),
    redis_client.clone(),
  ));

  // Example data to cache before the background task writes it to PostgreSQL.
  let new_user = User {
    id: 1,
    name: "John Doe".to_string(),
    email: "john.doe@example.com".to_string(),
  };

  write_user_to_cache(&mut redis_conn, &new_user).await?;

  println!("Background sync is running. Press Ctrl+C to stop.");
  tokio::signal::ctrl_c().await?;
  sync_task.abort();

  Ok(())
}

async fn write_user_to_cache(
  redis_conn: &mut redis::aio::MultiplexedConnection, // Updated connection type
  user: &User,
) -> Result<(), Box<dyn std::error::Error>> {
  // Serialize the user struct to JSON for Redis
  let user_json = serde_json::to_string(user)?;

  // Cache the user first. The background sync task will persist it later.
  let cache_key = format!("user:{}", user.id);
  redis_conn
    .set::<String, String, ()>(cache_key.clone(), user_json)
    .await?;
  redis_conn
    .expire::<String, ()>(cache_key, USER_CACHE_TTL_SECS)
    .await?;

  println!("User written to Redis cache.");

  Ok(())
}

async fn sync_redis_to_postgres_in_background(pg_pool: PgPool, redis_client: redis::Client) {
  let mut ticker = interval(Duration::from_secs(SYNC_INTERVAL_SECS));

  loop {
    ticker.tick().await;

    let mut redis_conn = match redis_client.get_multiplexed_async_connection().await {
      Ok(redis_conn) => redis_conn,
      Err(err) => {
        eprintln!("Failed to connect to Redis for background sync: {err}");
        continue;
      }
    };

    if let Err(err) = sync_redis_to_postgres(&pg_pool, &mut redis_conn).await {
      eprintln!("Failed to sync Redis to PostgreSQL: {err}");
    }
  }
}

async fn sync_redis_to_postgres(
  pg_pool: &Pool<Postgres>,
  redis_conn: &mut redis::aio::MultiplexedConnection, // Updated connection type
) -> Result<(), Box<dyn std::error::Error>> {
  // Get all keys matching the pattern "user:*"
  let keys: Vec<String> = redis_conn.keys("user:*").await?;

  for key in keys {
    // Get the user JSON string from Redis
    let user_json: String = redis_conn.get(&key).await?;

    // Deserialize JSON string to User struct
    let user: User = serde_json::from_str(&user_json)?;

    // Check if the user exists in PostgreSQL
    let existing_user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = $1")
      .bind(user.id)
      .fetch_one(pg_pool)
      .await?;

    if existing_user_count > 0 {
      // User exists in PostgreSQL, update the user
      sqlx::query("UPDATE users SET name = $1, email = $2 WHERE id = $3")
        .bind(&user.name)
        .bind(&user.email)
        .bind(user.id)
        .execute(pg_pool)
        .await?;
      println!("User with id {} updated in PostgreSQL.", user.id);
    } else {
      // User does not exist in PostgreSQL, insert the user
      sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $2, $3)")
        .bind(user.id)
        .bind(&user.name)
        .bind(&user.email)
        .execute(pg_pool)
        .await?;
      println!("User with id {} inserted into PostgreSQL.", user.id);
    }

    // Optionally delete the Redis key after syncing
    let _: () = redis_conn.del(&key).await?;
    println!(
      "User with id {} synced from Redis to PostgreSQL and deleted from Redis cache.",
      user.id
    );
  }

  Ok(())
}

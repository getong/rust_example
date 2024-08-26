use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Pool, Postgres};
use std::env;
use tokio;

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

  // Example data to write
  let new_user = User {
    id: 1,
    name: "John Doe".to_string(),
    email: "john.doe@example.com".to_string(),
  };

  // Write user to PostgreSQL and cache in Redis
  write_user(&pg_pool, &mut redis_conn, &new_user).await?;

  // Sync data from Redis to PostgreSQL
  sync_redis_to_postgres(&pg_pool, &mut redis_conn).await?;

  Ok(())
}

async fn write_user(
  pg_pool: &Pool<Postgres>,
  redis_conn: &mut redis::aio::MultiplexedConnection, // Updated connection type
  user: &User,
) -> Result<(), Box<dyn std::error::Error>> {
  // Serialize the user struct to JSON for Redis
  let user_json = serde_json::to_string(user)?;

  // Insert user into PostgreSQL
  sqlx::query!(
    "INSERT INTO users (id, name, email) VALUES ($1, $2, $3) ON CONFLICT (id) DO NOTHING",
    user.id,
    user.name,
    user.email
  )
  .execute(pg_pool)
  .await?;

  // Cache the user in Redis
  let cache_key = format!("user:{}", user.id);
  redis_conn
    .set::<String, String, ()>(cache_key.clone(), user_json)
    .await?;
  redis_conn.expire::<String, ()>(cache_key, 60 * 5).await?; // Cache for 5 minutes

  println!("User written to PostgreSQL and cached in Redis.");

  Ok(())
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
    let result = sqlx::query!("SELECT * FROM users WHERE id = $1", user.id)
      .fetch_optional(pg_pool)
      .await?;

    if result.is_some() {
      // User exists in PostgreSQL, update the user
      sqlx::query!(
        "UPDATE users SET name = $1, email = $2 WHERE id = $3",
        user.name,
        user.email,
        user.id
      )
      .execute(pg_pool)
      .await?;
      println!("User with id {} updated in PostgreSQL.", user.id);
    } else {
      // User does not exist in PostgreSQL, insert the user
      sqlx::query!(
        "INSERT INTO users (id, name, email) VALUES ($1, $2, $3)",
        user.id,
        user.name,
        user.email
      )
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

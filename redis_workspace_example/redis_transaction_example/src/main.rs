use redis::AsyncCommands;
use std::{env, error::Error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // Load environment variables
  dotenv::dotenv().ok();

  let redis_url = env::var("REDIS_URL").expect("REDIS_URL must be set");

  // Create Redis client and connection
  let redis_client = redis::Client::open(redis_url)?;
  let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;

  // Key and TTL settings
  let key = "my_counter";
  let ttl_seconds = 60;

  // Start a Redis transaction using the pipeline and atomic() function
  let _: () = redis::pipe()
    .atomic() // Makes the pipeline transactional
    .cmd("INCR") // First command to increment the counter
    .arg(key)
    .ignore() // Ignore the result of this command (not required)
    .cmd("EXPIRE") // Second command to set the TTL
    .arg(key)
    .arg(ttl_seconds)
    .ignore() // Ignore the result of this command (not required)
    .query_async(&mut redis_conn)
    .await?; // Execute the transaction

  // Retrieve and print the current value of the counter
  let current_value: i32 = redis_conn.get(key).await?;
  println!("The current value of '{}' is: {}", key, current_value);

  // Start a Redis transaction using the pipeline and atomic() function
  let result: Vec<i32> = redis::pipe()
    .atomic() // Makes the pipeline transactional
    .cmd("INCR") // First command to increment the counter
    .arg(key)
    .cmd("EXPIRE") // Second command to set the TTL
    .arg(key)
    .arg(ttl_seconds)
    .query_async(&mut redis_conn)
    .await?; // Execute the transaction

  println!("result is {:?}", result);

  // Retrieve and print the current value of the counter
  let current_value: i32 = redis_conn.get(key).await?;
  println!("The current value of '{}' is: {}", key, current_value);

  let ((k1, k2),): ((i32, i32),) = redis::pipe()
    .cmd("SET")
    .arg("key_1")
    .arg(42)
    .ignore()
    .cmd("SET")
    .arg("key_2")
    .arg(43)
    .ignore()
    .cmd("MGET")
    .arg(&["key_1", "key_2"])
    .query_async(&mut redis_conn)
    .await?;
  println!("k1: {}, k2: {}", k1, k2);

  Ok(())
}

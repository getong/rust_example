use redis::AsyncCommands;
use std::{env, error::Error};
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // Load environment variables
  dotenv::dotenv().ok();

  let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());

  // Create Redis client and connection
  let redis_client = redis::Client::open(redis_url)?;
  let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;

  // Define the Redis set key
  let set_key = "myset";

  // Add members to the set using SADD
  let _: () = redis_conn.sadd(set_key, "member1").await?;
  let _: () = redis_conn.sadd(set_key, "member2").await?;
  let _: () = redis_conn.sadd(set_key, "member3").await?;
  println!("Members added to the set '{}'.", set_key);

  // Retrieve all members of the set using SMEMBERS
  let members: Vec<String> = redis_conn.smembers(set_key).await?;
  println!("Members in the set '{}': {:?}", set_key, members);

  // Remove a member from the set using SREM
  let _: () = redis_conn.srem(set_key, "member2").await?;
  println!("Member 'member2' removed from the set '{}'.", set_key);

  // Retrieve all members of the set again to verify removal
  let members_after_removal: Vec<String> = redis_conn.smembers(set_key).await?;
  println!(
    "Members in the set '{}' after removal: {:?}",
    set_key, members_after_removal
  );

  Ok(())
}

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

  for i in 0 .. 10 {
    let key = format!("my_counter_{}", i);
    // let ttl_seconds = 60;

    // Start a Redis transaction using the pipeline and atomic() function
    let _: () = redis::pipe()
      .atomic()
      .cmd("set") // Makes the pipeline transactional
      .arg(&key) // First command to increment the counter
      .arg(i)
      .ignore() // Ignore the result of this command (not required)
      .cmd("SADD")
      .arg("myset")
      .arg(&key)
      .ignore() // Ignore the result of this command (not required)
      .query_async(&mut redis_conn)
      .await?; // Execute the transaction

    // Retrieve and print the current value of the counter
    let current_value: i32 = redis_conn.get(&key).await?;
    println!("The current value of '{}' is: {}", key, current_value);
  }

  // Define the Redis set key
  let set_key = "myset";

  // Add members to the set using SADD
  let _: () = redis_conn.sadd(set_key, "member1").await?;
  let _: () = redis_conn.sadd(set_key, "member2").await?;
  let _: () = redis_conn.sadd(set_key, "member3").await?;
  println!("Members added to the set '{}'.", set_key);

  // Retrieve all members of the set using SMEMBERS
  let members: Vec<String> = redis_conn.smembers(set_key).await?;
  // Attempt to retrieve the values of the keys in the set
  match redis_conn.mget::<_, Vec<Option<i32>>>(&members).await {
    Ok(times_list) => {
      println!("Retrieved values for set members: {:?}", times_list);
      members
        .iter()
        .zip(times_list.iter())
        .zip(times_list.iter())
        .for_each(|((member, time1), time2)| {
          println!(
            "Member '{}' has value {:?}, time : {:?}",
            member, time1, time2
          );
        });
    }
    Err(err) => {
      println!("Failed to retrieve values for set members: {}", err);
    }
  }
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

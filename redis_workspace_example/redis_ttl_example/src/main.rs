use std::env;

use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
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

  let redis_url = env::var("REDIS_URL").expect("REDIS_URL must be set");

  // Create Redis client and connection
  let redis_client = redis::Client::open(redis_url)?;
  let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;

  let new_user = User {
    id: 1,
    name: "John Doe".to_string(),
    email: "john.doe@example.com".to_string(),
  };

  let data: Vec<u8> = bincode::serde::encode_to_vec(&new_user, bincode::config::standard()).unwrap();

  // Set a key with a value and an expiration time of 60 seconds
  let _: () = redis_conn.set_ex("mykey", data, 60).await?;

  println!("Key 'mykey' set with value and TTL of 60 seconds.");

  // Retrieve the serialized data back from Redis
  match redis_conn.get::<_, Option<Vec<u8>>>("mykey").await {
    Ok(Some(data)) => {
      // Deserialize the data back into a User struct
      let (retrieved_user, _): (User, usize) = bincode::serde::decode_from_slice(&data, bincode::config::standard()).unwrap();
      println!("Retrieved user from Redis: {:?}", retrieved_user);
    }
    Ok(None) => {
      println!("Key 'mykey' not found in Redis.");
    }
    Err(err) => {
      println!("Failed to retrieve data from Redis: {}", err);
    }
  }
  let value = redis_conn.del::<_, i32>("mykey").await?;
  println!("delete value is {:?}", value);

  // let list = vec![1, 123];
  // let _: () = redis_conn.set("mykey", list).await?;
  // match redis_conn.get::<_, Option<Vec<i32>>>("mykey").await {
  //   Ok(Some(data)) => {
  //     // Deserialize the data back into a User struct
  //     println!(" get list , data is {:?}", data);
  //   }
  //   Ok(None) => {
  //     println!("Key 'mykey' not found in Redis.");
  //   }
  //   Err(err) => {
  //     println!("Failed to retrieve data from Redis: {}", err);
  //   }
  // }

  Ok(())
}

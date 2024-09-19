use redis::{aio::ConnectionManagerConfig, Client};

#[tokio::main]
async fn main() {
  let redis_url = "redis://bert:abc123@127.0.0.1:6379/";
  let redis_client = Client::open(redis_url).unwrap();

  let connection_manager_config = ConnectionManagerConfig::new()
    .set_number_of_retries(2)
    .set_max_delay(2000);

  if let Ok(mut redis_connection_manager) = redis_client
    .get_connection_manager_with_config(connection_manager_config)
    .await
  {
    for i in 1 .. 100 {
      let key = format!("abc_{}", i);
      println!(" the key is {}", key);
      if let Ok(_) = redis::pipe()
        .atomic()
        .set(&key, 1u8)
        .expire(&key, 60i64)
        .query_async::<()>(&mut redis_connection_manager)
        .await
      {
        println!("key {} set ok", key);
      }

      tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
  }
}

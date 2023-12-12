use fred::pool::RedisPool;
use fred::prelude::*;
use futures::stream::StreamExt;
// use std::iter::Iterator;
// use futures_util::stream::stream::StreamExt;
// use chrono::Utc;

// use fred::types::RedisValue;

#[tokio::main]
async fn main() -> Result<(), RedisError> {
  let config = RedisConfig {
    username: Some("bert".to_owned()),
    password: Some("abc123".to_owned()),
    ..RedisConfig::default()
  };

  let pool = RedisPool::new(config, 5)?;
  let policy = ReconnectPolicy::default();
  let _ = pool.connect(Some(policy));
  let _ = pool.wait_for_connect().await?;

  // let now = Utc::now().timestamp_millis();
  // let _:u8 = pool.next().hset(String::from("message::complete"), ("hello", RedisValue::Integer(now))).await?;
  // let hget_result :String = pool.next().hget(String::from("message::complete"), "content").await;
  // println!("hget_result:{:?}", hget_result);

  let client = pool.next();

  // use the pool like any other RedisClient
  let _ = client.get("foo").await?;
  let _: () = client.set("foo", "bar", None, None, false).await?;
  let foo_set_result: RedisValue = client.get("foo").await?;
  println!("foo_set_result:{:?}", foo_set_result);

  let mut scan_stream = client.scan("foo*", Some(10), None);
  while let Some(Ok(mut page)) = scan_stream.next().await {
    if let Some(keys) = page.take_results() {
      // println!("scan_result:{:?}", scan_result);
      // let client = page.create_client();

      for key in keys.into_iter() {
        let value: RedisValue = client.get(&key).await?;
        println!("Scanned {} -> {:?}", key.as_str_lossy(), value);
        println!("value:{:?}", value);
      }
    }
  }

  let _ = pool.quit_pool().await;
  Ok(())
}

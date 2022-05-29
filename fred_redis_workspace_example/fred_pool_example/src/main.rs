use fred::pool::RedisPool;
use fred::prelude::*;
// use chrono::Utc;

use fred::types::RedisValue;

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


    // use the pool like any other RedisClient
    let _ = pool.next().get("foo").await?;
    let _ = pool.next().set("foo", "bar", None, None, false).await?;
    let foo_set_result = pool.next().get("foo").await?;
    println!("foo_set_result:{:?}", foo_set_result);

    let _ = pool.quit_pool().await;
    Ok(())
}

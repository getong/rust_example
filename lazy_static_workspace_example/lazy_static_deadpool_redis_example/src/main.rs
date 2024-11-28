use arc_swap::ArcSwap;
use deadpool_redis::{redis::cmd, Config, Pool, Runtime};
use lazy_static::lazy_static;

lazy_static! {
  static ref GLOBAL_REDIS_SERVER: ArcSwap<Pool> = {
    let cfg = Config::from_url("redis://bert:abc123@127.0.0.1:6379/");
    match cfg.create_pool(Some(Runtime::Tokio1)) {
      Ok(pool) => ArcSwap::from_pointee(pool),
      Err(_) => panic!("connect to redis fail"),
    }
  };
}

#[tokio::main]
async fn main() {
  let pool = &**GLOBAL_REDIS_SERVER.load();
  {
    let mut conn = pool.get().await.unwrap();
    cmd("SET")
      .arg(&["deadpool/test_key", "72"])
      .query_async::<_, ()>(&mut conn)
      .await
      .unwrap();
  }
  {
    // let mut conn = (**GLOBAL_REDIS_SERVER.load()).get().await.unwrap();
    let mut conn = pool.get().await.unwrap();
    let value: String = cmd("GET")
      .arg(&["deadpool/test_key"])
      .query_async(&mut conn)
      .await
      .unwrap();
    assert_eq!(value, "72".to_string());
  }
}

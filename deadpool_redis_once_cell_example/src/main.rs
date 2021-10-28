use deadpool_redis::{redis::cmd, Config, Pool, Runtime};
use once_cell::sync;

static GLOBAL_REDIS_POOL: sync::Lazy<Pool> = sync::Lazy::new(|| {
    let cfg = Config::from_url("redis://bert:abc123@127.0.0.1/");
    cfg.create_pool(Some(Runtime::Tokio1)).unwrap()
});

#[tokio::main]
async fn main() {
    let mut conn = GLOBAL_REDIS_POOL.get().await.unwrap();

    cmd("SET")
        .arg(&["deadpool/test_key", "42"])
        .query_async::<_, ()>(&mut conn)
        .await
        .unwrap();

    let value: String = cmd("GET")
        .arg(&["deadpool/test_key"])
        .query_async(&mut conn)
        .await
        .unwrap();
    assert_eq!(value, "42".to_string());
}

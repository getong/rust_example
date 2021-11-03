use arc_swap::ArcSwap;
use deadpool_redis::{redis::cmd, Config, Pool, Runtime};
use once_cell::sync::Lazy;
use std::sync::Arc;

// must write like this, because pool can not be captured at once
static GLOBAL_REDIS_POOL_STRING: Lazy<ArcSwap<Option<Pool>>> =
    Lazy::new(|| ArcSwap::new(Arc::new(init_database_pool())));

fn init_database_pool() -> Option<Pool> {
    let cfg = Config::from_url("redis://bert:abc123@127.0.0.1:6379/");
    match cfg.create_pool(Some(Runtime::Tokio1)) {
        Ok(pool) => Some(pool),

        Err(_) => None,
    }
}

#[tokio::main]
async fn main() {
    match &**GLOBAL_REDIS_POOL_STRING.load() {
        Some(connection_info) => match connection_info.get().await {
            Ok(mut conn) => {
                let _ = cmd("SET")
                    .arg(&["deadpool/test_key", "72"])
                    .query_async::<_, ()>(&mut conn)
                    .await;
                let value: String = cmd("GET")
                    .arg(&["deadpool/test_key"])
                    .query_async(&mut conn)
                    .await
                    .unwrap();

                assert_eq!(value, "72".to_string());
                println!("72 is reached!");
            }
            _ => (),
        },
        _ => (),
    }
}

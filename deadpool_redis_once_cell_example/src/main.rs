use deadpool_redis::{redis::cmd, Config, Pool, Runtime};
use once_cell::sync::OnceCell;

static GLOBAL_REDIS_POOL: OnceCell<Pool> = OnceCell::new();

pub async fn initial_database() {
    if GLOBAL_REDIS_POOL.get().is_some() {
        return;
    }

    let cfg = Config::from_url("redis://bert:abc123@127.0.0.1:6379/");
    match cfg.create_pool(Some(Runtime::Tokio1)) {
        Ok(pool) => {
            let _ = GLOBAL_REDIS_POOL.set(pool);
        }
        Err(_) => panic!("connect to redis fail"),
    }
}

//static GLOBAL_REDIS_POOL: sync::Lazy<Pool> = sync::Lazy::new(|| {
//    let cfg = Config::from_url("redis://bert:abc123@127.0.0.1:6379/");
//    cfg.create_pool(Some(Runtime::Tokio1)).unwrap()
//});

#[tokio::main]
async fn main() {
    initial_database().await;

    match GLOBAL_REDIS_POOL.get() {
        Some(connection_info) => match connection_info.get().await {
            Ok(mut conn) => {
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
            _ => println!("database connection error"),
        },

        _ => println!("database connection error"),
    }
}

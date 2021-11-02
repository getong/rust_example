use arc_swap::ArcSwap;
use deadpool_redis::{redis::cmd, Config, Pool, Runtime};
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;
use std::sync::Arc;

// can not write like this, because pool is captured here
static GLOBAL_REDIS_POOL_STRING: Lazy<ArcSwap<Option<Pool>>> =
    Lazy::new(|| ArcSwap::new(Arc::new(init_database_pool())));

fn init_database_pool() -> Option<Pool> {
    let cfg = Config::from_url("redis://bert:abc123@127.0.0.1:6379/");
    match cfg.create_pool(Some(Runtime::Tokio1)) {
        Ok(pool) => Some(pool),

        Err(_) => None,
    }
}

static GLOBAL_REDIS_POOL: OnceCell<ArcSwap<Option<Pool>>> = OnceCell::new();

pub async fn initial_database() {
    if GLOBAL_REDIS_POOL.get().is_some() && GLOBAL_REDIS_POOL.get().unwrap().load().is_some() {
        println!(
            "GLOBAL_REDIS_POOL.get().unwrap().load().is_some(): {:?}",
            GLOBAL_REDIS_POOL.get().unwrap().load().is_some()
        );
        return;
    }

    let cfg = Config::from_url("redis://bert:abc123@127.0.0.1:6379/");
    match cfg.create_pool(Some(Runtime::Tokio1)) {
        Ok(pool) => {
            let _ = GLOBAL_REDIS_POOL.set(ArcSwap::from_pointee(Some(pool)));
        }
        Err(_) => panic!("connect to redis fail"),
    }
}

async fn reconnect_database(i: usize) {
    if let Ok(_) = GLOBAL_REDIS_POOL.set(ArcSwap::from_pointee(None)) {
        initial_database().await;
    } else {
        println!("not avaliable set , i is {}", i);
    }
}

#[tokio::main]
async fn main() {
    // use Lazy::new() method
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

    initial_database().await;
    for i in 1.. {
        match GLOBAL_REDIS_POOL.get() {
            Some(connection_info) => match &**connection_info.load() {
                Some(conn) => match conn.get().await {
                    Ok(mut conn) => {
                        if let Ok(_) = cmd("SET")
                            .arg(&["deadpool/test_key", "62"])
                            .query_async::<_, ()>(&mut conn)
                            .await
                        {
                            // println!("set ok");
                            let value: String = cmd("GET")
                                .arg(&["deadpool/test_key"])
                                .query_async(&mut conn)
                                .await
                                .unwrap_or("62".to_string());

                            assert_eq!(value, "62".to_string());
                        } else {
                            reconnect_database(i).await;
                        }
                    }

                    //  pool 不能获取连接，重新初始化连接过去redis-server
                    _ => {
                        println!("pool no child, i:{}", i);
                        reconnect_database(i).await;
                    }
                },

                _ => {
                    println!("pool not init, i:{}", i);
                    reconnect_database(i).await;
                }
            },
            _ => {
                reconnect_database(i).await;
            }
        }
    }
}

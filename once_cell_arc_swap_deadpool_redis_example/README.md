# once_cell, arc_swap and deadpool_redis example
The code is originally copied from [Crate deadpool_redis](https://docs.rs/deadpool-redis/0.10.0/deadpool_redis/)

another code example is copied from [One global variable for MySQL connection](https://users.rust-lang.org/t/one-global-variable-for-mysql-connection/49063)

## run commands

``` rust
redis-server redis.conf
cargo run -p once_cell_lazy_arc_swap_deadpool_redis_example
cargo run -p once_cell_arc_swap_deadpool_redis_example2
```

``` rust
use once_cell::sync::OnceCell;

static MONGODB: OnceCell<Database> = OnceCell::new();

pub async fn initialize() {
    if MONGODB.get().is_some() {
        return;
    }

    if let Ok(token) = env::var("CONNECTION") {
        if let Ok(client_options) = ClientOptions::parse(token.as_str()).await {
            if let Ok(client) = Client::with_options(client_options) {
                let _ = MONGODB.set(client.database("my_db"));
            }
        }
    }
}
```

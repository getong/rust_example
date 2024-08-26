use redis::Client;
// use redis::{RedisWrite, ToRedisArgs};

#[tokio::main]
async fn main() {
  let redis_url = "redis://bert:abc123@127.0.0.1:6379/";
  let redis_client = Client::open(redis_url).unwrap();
  let mut redis_connection_manager = redis_client
    .get_connection_manager()
    .await
    .expect("can't create redis connection manager");

  let _: () = redis::pipe()
    .atomic()
    .set("abc", 1u8)
    .expire("abc", 60i64)
    .query_async(&mut redis_connection_manager)
    .await
    .unwrap();
}

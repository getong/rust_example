use fred::prelude::*;

#[tokio::main]
async fn main() -> Result<(), RedisError> {
  let pool = Builder::default_centralized().build_pool(5)?;
  pool.init().await?;

  // all client types, including `RedisPool`, implement the same command interface traits so callers
  // can often use them interchangeably. in this example each command below will be sent
  // round-robin to the underlying 5 clients.
  assert!(pool.get::<Option<String>, _>("foo").await?.is_none());
  pool.set("foo", "bar", None, None, false).await?;
  assert_eq!(pool.get::<String, _>("foo").await?, "bar");

  pool.del("foo").await?;
  // interact with specific clients via next(), last(), or clients()
  let pipeline = pool.next().pipeline();
  pipeline.incr("foo").await?;
  pipeline.incr("foo").await?;
  assert_eq!(pipeline.last::<i64>().await?, 2);

  for client in pool.clients() {
    println!(
      "{} connected to {:?}",
      client.id(),
      client.active_connections().await?
    );
  }

  pool.quit().await?;
  Ok(())
}

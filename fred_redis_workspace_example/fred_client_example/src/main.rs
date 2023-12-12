use fred::prelude::*;

#[tokio::main]
async fn main() -> Result<(), RedisError> {
  let default_config = RedisConfig::default();
  let config = RedisConfig {
    username: Some("bert".to_owned()),
    password: Some("abc123".to_owned()),
    ..default_config
  };
  let policy = ReconnectPolicy::default();
  let client = RedisClient::new(config);

  // connect to the server, returning a handle to a task that drives the connection
  let jh = client.connect(Some(policy));
  // wait for the client to connect
  let _ = client.wait_for_connect().await?;
  let _ = client.flushall(false).await?;

  // convert responses to many common Rust types
  let foo: Option<String> = client.get("foo").await?;
  assert_eq!(foo, None);

  let _: () = client.set("foo", "bar", None, None, false).await?;
  // or use turbofish to declare types. the first type is always the response.
  println!(
    "Foo: {:?}",
    client.get::<String, _>("foo".to_owned()).await?
  );
  // or use a lower level interface for responses to defer parsing, etc
  let foo: RedisValue = client.get("foo").await?;
  assert!(foo.is_string());

  let _ = client.quit().await?;
  // and/or wait for the task driving the connection to finish
  let _ = jh.await;
  Ok(())
}

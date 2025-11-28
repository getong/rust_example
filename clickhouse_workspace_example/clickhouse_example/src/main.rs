use std::env;

use clickhouse::{sql, Client, Row};
use serde_derive::{Deserialize, Serialize};

#[derive(Row, Deserialize, Serialize)]
struct MyRow<'a> {
  no: u32,
  name: &'a str,
}

fn env_or(key: &str, default: &str) -> String {
  env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Defaults align with the local Docker setup; override via CH_URL/CH_USER/CH_PASSWORD.
  let client = Client::default()
    .with_url(env_or("CH_URL", "http://localhost:8123"))
    .with_user(env_or("CH_USER", "default"))
    .with_password(env_or("CH_PASSWORD", "changeme"));

  client
    .query("CREATE DATABASE IF NOT EXISTS ?")
    .bind(sql::Identifier("test"))
    .execute()
    .await
    .expect("cannot create db");

  let client = client.with_database("test");

  client
    .query(
      "
            CREATE TABLE IF NOT EXISTS some(no UInt32, name LowCardinality(String))
            ENGINE = MergeTree
            ORDER BY no
        ",
    )
    .execute()
    .await?;

  // Insertion is async; await the future before writing rows. Type is explicit for inference.
  let mut insert = client.insert::<MyRow<'static>>("some").await?;
  insert.write(&MyRow { no: 0, name: "foo" }).await?;
  insert.write(&MyRow { no: 1, name: "bar" }).await?;
  Ok(insert.end().await?)
}

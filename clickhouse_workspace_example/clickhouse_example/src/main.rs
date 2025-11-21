use clickhouse::{sql, Client, Row};
use serde_derive::{Deserialize, Serialize};

#[derive(Row, Deserialize, Serialize)]
struct MyRow<'a> {
  no: u32,
  name: &'a str,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let client = Client::default()
    .with_url("http://192.168.5.203:8123")
    .with_user("default")
    .with_password("aeYee8ah");

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

use std::{env, time::UNIX_EPOCH};

use clickhouse::{Client, Row, error::Result as ChResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Row)]
struct Event {
  timestamp: u128,
  message: String,
}

fn env_or(key: &str, default: &str) -> String {
  env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
  let url = env_or("CH_URL", "http://localhost:8123"); // kept for backward compat
  let user = env_or("CH_USER", "default");
  let password = env::var("CH_PASSWORD").unwrap_or_else(|_| "changeme".to_string());
  let database = env_or("CH_DB", "test");
  let node_urls = env_or("CH_NODES", &format!("{url},http://localhost:8124"));
  let cluster = env_or("CH_CLUSTER", "ch_cluster");

  let clients = build_clients(&node_urls, &user, &password, &database);
  let primary = clients
    .get(0)
    .expect("at least one node url required")
    .clone();

  create_tables(&clients, &cluster, &database).await?;
  insert_sample(&primary).await?;
  read_back(&primary).await?;

  Ok(())
}

fn build_clients(urls: &str, user: &str, password: &str, db: &str) -> Vec<Client> {
  urls
    .split(',')
    .filter(|s| !s.trim().is_empty())
    .map(|url| {
      Client::default()
        .with_url(url.trim())
        .with_user(user)
        .with_password(password)
        .with_database(db.to_string())
    })
    .collect()
}

async fn create_tables(clients: &[Client], cluster: &str, db: &str) -> ChResult<()> {
  for client in clients {
    let create_db = format!("CREATE DATABASE IF NOT EXISTS {db}");
    client.query(&create_db).execute().await?;

    let create_local = format!(
      "
      CREATE TABLE IF NOT EXISTS {db}.cluster_events (
        timestamp UInt128,
        message   String
      )
      ENGINE = MergeTree
      ORDER BY timestamp
      "
    );
    client.query(&create_local).execute().await?;

    let create_dist = format!(
      "
      CREATE TABLE IF NOT EXISTS {db}.cluster_events_dist
      AS {db}.cluster_events
      ENGINE = Distributed({cluster}, {db}, cluster_events, cityHash64(timestamp))
      "
    );
    client.query(&create_dist).execute().await?;
  }
  Ok(())
}

async fn insert_sample(client: &Client) -> ChResult<()> {
  let mut insert = client.insert::<Event>("cluster_events_dist").await?;
  insert
    .write(&Event {
      timestamp: now_nanos(),
      message: "hello from shard-aware insert".into(),
    })
    .await?;
  insert
    .write(&Event {
      timestamp: now_nanos() + 1,
      message: "another row distributed across the cluster".into(),
    })
    .await?;
  insert.end().await
}

async fn read_back(client: &Client) -> ChResult<()> {
  let events = client
    .query("SELECT ?fields FROM cluster_events_dist ORDER BY timestamp")
    .fetch_all::<Event>()
    .await?;

  println!("Read {} events across the cluster:", events.len());
  for event in events {
    println!("{event:?}");
  }
  Ok(())
}

fn now_nanos() -> u128 {
  UNIX_EPOCH
    .elapsed()
    .expect("invalid system time")
    .as_nanos()
}

use std::{env, time::UNIX_EPOCH};

use clickhouse::{error::Result as ChResult, Client, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Row)]
struct Event {
  user_id: u64,
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
  // Default to cluster password ("changeme"); override via CH_PASSWORD
  let password = env_or("CH_PASSWORD", "changeme");
  let database = env_or("CH_DB", "test");
  let node_urls = env_or(
    "CH_NODES",
    &format!("{url},http://localhost:8124,http://localhost:8125,http://localhost:8126"),
  );
  let cluster = env_or("CH_CLUSTER", "ch_cluster");

  let clients = build_clients(&node_urls, &user, &password, &database);
  let primary = clients
    .get(0)
    .expect("at least one node url required")
    .clone();

  // Health check before proceeding
  println!("Performing health check on cluster nodes...");
  check_cluster_health(&clients).await?;

  println!("\nCreating tables...");
  create_tables(&clients, &cluster, &database).await?;

  // Verify tables were created on all nodes
  println!("\nVerifying tables on all nodes...");
  verify_tables(&clients, &database).await?;

  println!("\nInserting sample data...");
  insert_sample(&primary).await?;

  // Wait a bit for replication to propagate
  tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

  println!("\nReading back data...");
  read_back(&primary).await?;

  println!("\nChecking data distribution across shards...");
  check_data_distribution(&clients, &database).await?;

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
  // Create local replicated tables on each node
  for client in clients {
    let create_db = format!("CREATE DATABASE IF NOT EXISTS {db}");
    client.query(&create_db).execute().await?;

    // Use ReplicatedMergeTree for automatic replication
    // The {shard} and {replica} macros are substituted from cluster config
    let create_local = format!(
      "
      CREATE TABLE IF NOT EXISTS {db}.cluster_events (
        user_id   UInt64,
        timestamp UInt128,
        message   String
      )
      ENGINE = ReplicatedMergeTree('/clickhouse/tables/{{shard}}/{db}/cluster_events', \
       '{{replica}}')
      ORDER BY (user_id, timestamp)
      "
    );
    client.query(&create_local).execute().await?;
  }

  // Create distributed table only on the first (primary) node
  // Use user_id as sharding key to ensure same user data goes to same shard
  let primary = &clients[0];
  let create_dist = format!(
    "
    CREATE TABLE IF NOT EXISTS {db}.cluster_events_dist
    AS {db}.cluster_events
    ENGINE = Distributed({cluster}, {db}, cluster_events, cityHash64(user_id))
    "
  );
  primary.query(&create_dist).execute().await?;

  Ok(())
}

async fn insert_sample(client: &Client) -> ChResult<()> {
  let mut insert = client.insert::<Event>("cluster_events_dist").await?;

  // Insert events for different users - will be distributed by user_id
  for user_id in 1001 ..= 1010 {
    insert
      .write(&Event {
        user_id,
        timestamp: now_nanos() + user_id as u128,
        message: format!("Event from user {}", user_id),
      })
      .await?;
  }

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

// Health check: verify all nodes are accessible and responding
async fn check_cluster_health(clients: &[Client]) -> ChResult<()> {
  for (idx, client) in clients.iter().enumerate() {
    match client.query("SELECT version()").fetch_one::<String>().await {
      Ok(version) => {
        println!(
          "✓ Node {} is healthy (ClickHouse version: {})",
          idx + 1,
          version
        );
      }
      Err(e) => {
        eprintln!("✗ Node {} health check failed: {}", idx + 1, e);
        return Err(e);
      }
    }
  }

  // Check cluster configuration
  let primary = &clients[0];
  let cluster_info = primary
    .query(
      "SELECT cluster, shard_num, replica_num, host_name FROM system.clusters WHERE cluster = \
       'ch_cluster' ORDER BY shard_num, replica_num",
    )
    .fetch_all::<(String, u32, u32, String)>()
    .await?;

  println!("\nCluster configuration:");
  for (cluster, shard, replica, host) in cluster_info {
    println!(
      "  Cluster: {}, Shard: {}, Replica: {}, Host: {}",
      cluster, shard, replica, host
    );
  }

  Ok(())
}

// Verify tables exist on all nodes
async fn verify_tables(clients: &[Client], db: &str) -> ChResult<()> {
  for (idx, client) in clients.iter().enumerate() {
    // Check local table
    let local_exists = client
      .query(&format!(
        "SELECT count() FROM system.tables WHERE database = '{}' AND name = 'cluster_events'",
        db
      ))
      .fetch_one::<u64>()
      .await?;

    if local_exists == 0 {
      eprintln!(
        "✗ Local table 'cluster_events' not found on node {}",
        idx + 1
      );
      return Err(clickhouse::error::Error::Custom(format!(
        "Table verification failed on node {}",
        idx + 1
      )));
    }

    // Check table engine
    let engine = client
      .query(&format!(
        "SELECT engine FROM system.tables WHERE database = '{}' AND name = 'cluster_events'",
        db
      ))
      .fetch_one::<String>()
      .await?;

    println!(
      "✓ Node {}: table 'cluster_events' exists (Engine: {})",
      idx + 1,
      engine
    );
  }

  // Verify distributed table on primary node
  let dist_exists = clients[0]
    .query(&format!(
      "SELECT count() FROM system.tables WHERE database = '{}' AND name = 'cluster_events_dist'",
      db
    ))
    .fetch_one::<u64>()
    .await?;

  if dist_exists > 0 {
    println!("✓ Distributed table 'cluster_events_dist' exists on primary node");
  } else {
    eprintln!("✗ Distributed table 'cluster_events_dist' not found");
  }

  Ok(())
}

// Check how data is distributed across shards
async fn check_data_distribution(clients: &[Client], db: &str) -> ChResult<()> {
  for (idx, client) in clients.iter().enumerate() {
    let count = client
      .query(&format!("SELECT count() FROM {}.cluster_events", db))
      .fetch_one::<u64>()
      .await?;

    println!("Node {} local table has {} rows", idx + 1, count);
  }

  // Check total via distributed table
  let total = clients[0]
    .query(&format!("SELECT count() FROM {}.cluster_events_dist", db))
    .fetch_one::<u64>()
    .await?;

  println!(
    "Total rows across cluster (via distributed table): {}",
    total
  );

  // Show sample of data distribution by user_id
  let distribution = clients[0]
    .query(&format!(
      "SELECT user_id, count() as cnt FROM {}.cluster_events_dist GROUP BY user_id ORDER BY \
       user_id",
      db
    ))
    .fetch_all::<(u64, u64)>()
    .await?;

  println!("\nData distribution by user_id:");
  for (user_id, count) in distribution {
    println!("  user_id {}: {} events", user_id, count);
  }

  Ok(())
}

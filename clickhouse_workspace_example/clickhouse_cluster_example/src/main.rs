use std::{
  env,
  sync::atomic::{AtomicUsize, Ordering},
  time::UNIX_EPOCH,
};

use clickhouse::{error::Result as ChResult, Client, Compression, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Row)]
struct Event {
  user_id: u64,
  timestamp: u128,
  message: String,
}

struct ClientPool {
  clients: Vec<Client>,
  cursor: AtomicUsize,
}

impl ClientPool {
  fn new(clients: Vec<Client>) -> Self {
    assert!(
      !clients.is_empty(),
      "at least one ClickHouse node URL is required"
    );
    Self {
      clients,
      cursor: AtomicUsize::new(0),
    }
  }

  fn all(&self) -> &[Client] {
    &self.clients
  }

  /// Round-robin pick of the next client; returns (node_idx, Client).
  fn next(&self) -> (usize, Client) {
    let current = self.cursor.fetch_add(1, Ordering::Relaxed);
    let idx = current % self.clients.len();
    (idx, self.clients[idx].clone())
  }
}

// fn env_or(key: &str, default: &str) -> String {
//  env::var(key).unwrap_or_else(|_| default.to_string())
//}

fn env_first<'a>(keys: &[&'a str], default: &str) -> String {
  keys
    .iter()
    .find_map(|k| env::var(k).ok())
    .unwrap_or_else(|| default.to_string())
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
  // Prefer official ClickHouse env names, fallback to legacy CH_* for compatibility.
  let url = env_first(&["CLICKHOUSE_URL", "CH_URL"], "http://localhost:8123");
  let user = env_first(&["CLICKHOUSE_USER", "CH_USER"], "default");
  let password = env_first(&["CLICKHOUSE_PASSWORD", "CH_PASSWORD"], "changeme");
  let database = env_first(&["CLICKHOUSE_DATABASE", "CH_DB"], "test");
  let node_urls = env_first(
    &["CLICKHOUSE_NODES", "CH_NODES"],
    &format!("{url},http://localhost:8124,http://localhost:8125,http://localhost:8126"),
  );
  let cluster = env_first(&["CLICKHOUSE_CLUSTER", "CH_CLUSTER"], "ch_cluster");

  let pool = ClientPool::new(build_clients(&node_urls, &user, &password, &database));

  // Health check before proceeding
  println!("Performing health check on cluster nodes...");
  check_cluster_health(pool.all(), &cluster).await?;

  println!("\nCreating tables...");
  create_tables(pool.all(), &cluster, &database).await?;

  // Verify tables were created on all nodes
  println!("\nVerifying tables on all nodes...");
  verify_tables(pool.all(), &database).await?;

  let (writer_idx, writer) = pool.next();
  println!(
    "\nInserting sample data using node {} (round-robin load balanced)...",
    writer_idx + 1
  );
  insert_sample(&writer).await?;

  // Wait a bit for replication to propagate
  tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

  let (reader_idx, reader) = pool.next();
  println!(
    "\nReading back data using node {} (round-robin load balanced)...",
    reader_idx + 1
  );
  read_back(&reader).await?;

  println!("\nChecking data distribution across shards...");
  let (dist_idx, dist_client) = pool.next();
  println!(
    "Using node {} for distributed table checks (round-robin load balanced)...",
    dist_idx + 1
  );
  check_data_distribution(pool.all(), &dist_client, &database).await?;

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
        // Use LZ4 compression (default feature); matches official ClickHouse Rust client docs.
        .with_compression(Compression::Lz4)
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

  // Create distributed table on every node so any node can be used via load balancing.
  // Use user_id as sharding key to ensure same user data goes to same shard.
  let create_dist = format!(
    "
    CREATE TABLE IF NOT EXISTS {db}.cluster_events_dist
    AS {db}.cluster_events
    ENGINE = Distributed({cluster}, {db}, cluster_events, cityHash64(user_id))
    "
  );
  for client in clients {
    client.query(&create_dist).execute().await?;
  }

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

async fn check_cluster_health(clients: &[Client], cluster: &str) -> ChResult<()> {
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
      "SELECT cluster, shard_num, replica_num, host_name FROM system.clusters WHERE cluster = ? \
       ORDER BY shard_num, replica_num",
    )
    .bind(cluster)
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

    // Verify distributed table on each node for load-balanced access.
    let dist_exists = client
      .query(&format!(
        "SELECT count() FROM system.tables WHERE database = '{}' AND name = 'cluster_events_dist'",
        db
      ))
      .fetch_one::<u64>()
      .await?;

    if dist_exists > 0 {
      println!(
        "✓ Node {}: distributed table 'cluster_events_dist' exists",
        idx + 1
      );
    } else {
      eprintln!(
        "✗ Node {}: distributed table 'cluster_events_dist' not found",
        idx + 1
      );
      return Err(clickhouse::error::Error::Custom(format!(
        "Distributed table verification failed on node {}",
        idx + 1
      )));
    }
  }

  Ok(())
}

// Check how data is distributed across shards
async fn check_data_distribution(
  clients: &[Client],
  dist_client: &Client,
  db: &str,
) -> ChResult<()> {
  for (idx, client) in clients.iter().enumerate() {
    let count = client
      .query(&format!("SELECT count() FROM {}.cluster_events", db))
      .fetch_one::<u64>()
      .await?;

    println!("Node {} local table has {} rows", idx + 1, count);
  }

  // Check total via distributed table
  let total = dist_client
    .query(&format!("SELECT count() FROM {}.cluster_events_dist", db))
    .fetch_one::<u64>()
    .await?;

  println!(
    "Total rows across cluster (via distributed table): {}",
    total
  );

  // Show sample of data distribution by user_id
  let distribution = dist_client
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

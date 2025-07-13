//! Simple main application for testing the distributed system
//!
//! This provides basic functionality to test the chitchat + openraft integration

use std::{
  net::{IpAddr, Ipv4Addr, SocketAddr},
  time::Duration,
};

use chitchat_openraft_axum_example::distributed::{
  cluster::{Cluster, ClusterConfig},
  dht::DhtServer,
  member::Service,
};
use tokio::time::sleep;
use tracing::{Level, info};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Initialize logging
  tracing_subscriber::fmt().with_max_level(Level::INFO).init();

  info!("Starting simple distributed system test");

  // Create a test cluster with one DHT node
  let chitchat_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 10001);
  let dht_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);

  let cluster_config = ClusterConfig {
    chitchat_id: "test-node-1".to_string(),
    chitchat_listen_addr: chitchat_addr,
    seed_nodes: vec![],
    heartbeat_interval: Duration::from_secs(1),
    marked_for_deletion_grace_period: Duration::from_secs(30),
  };

  let dht_service = Service::Dht {
    host: dht_addr,
    shard: 0,
  };

  // Start cluster
  let mut cluster = Cluster::new(cluster_config, dht_service);
  cluster.start().await?;

  // Start DHT server
  let mut dht_server = DhtServer::new(1, 0, dht_addr);
  dht_server.start().await?;

  // Mark as ready
  cluster.mark_ready().await?;

  info!("System started successfully");

  // Give some time to stabilize
  sleep(Duration::from_secs(2)).await;

  // Update and check cluster state
  cluster.update_members().await?;
  let members = cluster.members();
  info!("Cluster has {} members", members.len());

  for member in members.values() {
    info!("Member: {}", member.service);
  }

  // Test DHT operations
  use chitchat_openraft_axum_example::distributed::dht::DhtRequest;

  let put_request = DhtRequest::Put {
    key: "test_key".to_string(),
    value: "test_value".to_string(),
  };

  match dht_server.handle_request(put_request).await {
    Ok(response) => info!("PUT response: {:?}", response),
    Err(e) => info!("PUT error: {}", e),
  }

  let get_request = DhtRequest::Get {
    key: "test_key".to_string(),
  };

  match dht_server.handle_request(get_request).await {
    Ok(response) => info!("GET response: {:?}", response),
    Err(e) => info!("GET error: {}", e),
  }

  info!("DHT has {} entries", dht_server.size().await);

  // Clean shutdown
  cluster.stop().await;
  info!("System stopped");

  Ok(())
}

//! Example application demonstrating the Stract-inspired architecture
//!
//! This example shows how to:
//! 1. Set up a cluster with chitchat for membership
//! 2. Create DHT services for consistent distributed storage
//! 3. Integrate both systems following the Stract pattern

use std::{
  net::{IpAddr, Ipv4Addr, SocketAddr},
  time::Duration,
};

use chitchat_openraft_axum_example::distributed::{
  cluster::{Cluster, ClusterConfig},
  dht::{DhtClient, DhtRequest, DhtResponse, DhtServer},
  member::Service,
};
use tokio::time::sleep;
use tracing::{Level, info};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Initialize logging
  tracing_subscriber::fmt().with_max_level(Level::INFO).init();

  info!("Starting Stract-inspired distributed system example");

  // Example 1: Start a single node cluster
  run_single_node_example().await?;

  // Example 2: Start multiple nodes
  run_multi_node_example().await?;

  info!("Example completed successfully");
  Ok(())
}

/// Example with a single node running both API and DHT services
async fn run_single_node_example() -> Result<(), Box<dyn std::error::Error>> {
  info!("=== Single Node Example ===");

  let listen_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 10001);
  let dht_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);

  // Create cluster configuration
  let cluster_config = ClusterConfig {
    chitchat_id: "node-1".to_string(),
    chitchat_listen_addr: listen_addr,
    seed_nodes: vec![], // First node, no seeds
    heartbeat_interval: Duration::from_secs(1),
    marked_for_deletion_grace_period: Duration::from_secs(30),
  };

  // Create DHT service
  let dht_service = Service::Dht {
    host: dht_addr,
    shard: 0,
  };

  // Start cluster
  let mut cluster = Cluster::new(cluster_config, dht_service.clone());
  cluster.start().await?;

  // Start DHT server
  let mut dht_server = DhtServer::new(1, 0, dht_addr);
  dht_server.start().await?;

  // Mark the service as ready
  cluster.mark_ready().await?;

  // Give some time for the cluster to stabilize
  sleep(Duration::from_secs(2)).await;

  // Update member registry
  let cluster_ref = &mut cluster;
  cluster_ref.update_members().await?;

  // Check cluster state
  let members = cluster.members();
  info!("Cluster has {} members", members.len());

  for member in members.values() {
    info!(
      "Member: ID={}, Service={}, Ready={}",
      member.id,
      member.service,
      member.is_ready()
    );
  }

  // Test DHT operations
  let put_request = DhtRequest::Put {
    key: "hello".to_string(),
    value: "world".to_string(),
  };
  let response = dht_server.handle_request(put_request).await?;
  info!("PUT response: {:?}", response);

  let get_request = DhtRequest::Get {
    key: "hello".to_string(),
  };
  let response = dht_server.handle_request(get_request).await?;
  info!("GET response: {:?}", response);

  // Get DHT state
  let state = dht_server.get_state().await;
  info!("DHT state: {:?}", state);

  // Stop the cluster
  cluster.stop().await;

  info!("Single node example completed");
  Ok(())
}

/// Example with multiple nodes forming a cluster
async fn run_multi_node_example() -> Result<(), Box<dyn std::error::Error>> {
  info!("=== Multi Node Example ===");

  // Node 1: DHT service
  let node1_listen = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 10002);
  let node1_dht = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8082);

  // Node 2: API service
  let node2_listen = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 10003);
  let node2_api = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8083);

  // Node 3: DHT service (different shard)
  let node3_listen = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 10004);
  let node3_dht = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8084);

  // Start Node 1 (DHT shard 0)
  let cluster_config1 = ClusterConfig {
    chitchat_id: "node-1".to_string(),
    chitchat_listen_addr: node1_listen,
    seed_nodes: vec![],
    heartbeat_interval: Duration::from_secs(1),
    marked_for_deletion_grace_period: Duration::from_secs(30),
  };

  let dht_service1 = Service::Dht {
    host: node1_dht,
    shard: 0,
  };

  let mut cluster1 = Cluster::new(cluster_config1, dht_service1);
  cluster1.start().await?;

  // Start Node 2 (API service)
  let cluster_config2 = ClusterConfig {
    chitchat_id: "node-2".to_string(),
    chitchat_listen_addr: node2_listen,
    seed_nodes: vec![node1_listen], // Join node 1
    heartbeat_interval: Duration::from_secs(1),
    marked_for_deletion_grace_period: Duration::from_secs(30),
  };

  let api_service = Service::Api { host: node2_api };

  let mut cluster2 = Cluster::new(cluster_config2, api_service);
  cluster2.start().await?;

  // Start Node 3 (DHT shard 1)
  let cluster_config3 = ClusterConfig {
    chitchat_id: "node-3".to_string(),
    chitchat_listen_addr: node3_listen,
    seed_nodes: vec![node1_listen], // Join node 1
    heartbeat_interval: Duration::from_secs(1),
    marked_for_deletion_grace_period: Duration::from_secs(30),
  };

  let dht_service3 = Service::Dht {
    host: node3_dht,
    shard: 1,
  };

  let mut cluster3 = Cluster::new(cluster_config3, dht_service3);
  cluster3.start().await?;

  // Mark all services as ready
  cluster1.mark_ready().await?;
  cluster2.mark_ready().await?;
  cluster3.mark_ready().await?;

  // Give time for cluster formation
  sleep(Duration::from_secs(3)).await;

  // Update member registries
  cluster1.update_members().await?;
  cluster2.update_members().await?;
  cluster3.update_members().await?;

  // Check cluster state from node 1's perspective
  let members = cluster1.members();
  info!(
    "Cluster has {} members from node 1's perspective",
    members.len()
  );

  for member in members.values() {
    info!(
      "Member: ID={}, Service={}, Ready={}",
      member.id,
      member.service,
      member.is_ready()
    );
  }

  // Get DHT members
  let dht_members = cluster1.get_dht_members();
  info!("DHT members: {}", dht_members.len());

  for member in &dht_members {
    info!(
      "DHT Member: Shard={:?}, Host={}",
      member.service.shard(),
      member.service.host()
    );
  }

  // Get API members
  let api_members = cluster1.get_api_members();
  info!("API members: {}", api_members.len());

  // Test DHT client functionality
  let mut dht_client = DhtClient::new();

  // Add shard servers to client
  dht_client.add_shard_servers(0, vec![node1_dht]);
  dht_client.add_shard_servers(1, vec![node3_dht]);

  // Test shard calculation
  let shard_for_key1 = dht_client.calculate_shard("key1");
  let shard_for_key2 = dht_client.calculate_shard("key2");
  info!("Key 'key1' maps to shard {}", shard_for_key1);
  info!("Key 'key2' maps to shard {}", shard_for_key2);

  // Attempt to perform operations (would work with full HTTP implementation)
  if let Err(e) = dht_client
    .put("test_key".to_string(), "test_value".to_string())
    .await
  {
    info!("PUT operation would work with full implementation: {}", e);
  }

  // Stop all clusters
  cluster1.stop().await;
  cluster2.stop().await;
  cluster3.stop().await;

  info!("Multi node example completed");
  Ok(())
}

/// Example of starting a dedicated DHT node
#[allow(dead_code)]
async fn start_dht_node(
  node_id: u32,
  shard_id: u32,
  chitchat_port: u16,
  dht_port: u16,
  seed_nodes: Vec<SocketAddr>,
) -> Result<(Cluster, DhtServer), Box<dyn std::error::Error>> {
  let chitchat_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), chitchat_port);
  let dht_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), dht_port);

  let cluster_config = ClusterConfig {
    chitchat_id: format!("dht-node-{}", node_id),
    chitchat_listen_addr: chitchat_addr,
    seed_nodes,
    heartbeat_interval: Duration::from_secs(1),
    marked_for_deletion_grace_period: Duration::from_secs(30),
  };

  let dht_service = Service::Dht {
    host: dht_addr,
    shard: shard_id,
  };

  let mut cluster = Cluster::new(cluster_config, dht_service);
  cluster.start().await?;

  let mut dht_server = DhtServer::new(node_id, shard_id, dht_addr);
  dht_server.start().await?;

  cluster.mark_ready().await?;

  info!(
    "DHT node {} started for shard {} on {}",
    node_id, shard_id, dht_addr
  );

  Ok((cluster, dht_server))
}

/// Example of starting an API node
#[allow(dead_code)]
async fn start_api_node(
  node_id: u32,
  chitchat_port: u16,
  api_port: u16,
  seed_nodes: Vec<SocketAddr>,
) -> Result<Cluster, Box<dyn std::error::Error>> {
  let chitchat_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), chitchat_port);
  let api_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), api_port);

  let cluster_config = ClusterConfig {
    chitchat_id: format!("api-node-{}", node_id),
    chitchat_listen_addr: chitchat_addr,
    seed_nodes,
    heartbeat_interval: Duration::from_secs(1),
    marked_for_deletion_grace_period: Duration::from_secs(30),
  };

  let api_service = Service::Api { host: api_addr };

  let mut cluster = Cluster::new(cluster_config, api_service);
  cluster.start().await?;
  cluster.mark_ready().await?;

  info!("API node {} started on {}", node_id, api_addr);

  Ok(cluster)
}

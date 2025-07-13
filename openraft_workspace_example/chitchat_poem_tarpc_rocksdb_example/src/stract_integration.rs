// Stract-inspired integration of chitchat + openraft
// This module demonstrates how to use chitchat for service discovery
// while using openraft for consensus, similar to how Stract implements it

use std::{collections::BTreeMap, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Result;

use crate::{
  chitchat_cluster::{ChitchatCluster, ClusterMember, ServiceType},
  common::{Api, Opt},
  ExampleRaft, Node, NodeId,
};

pub struct DistributedNode {
  pub raft_node_id: NodeId,
  pub raft: ExampleRaft,
  pub chitchat_cluster: ChitchatCluster,
  pub api: Api,
}

impl DistributedNode {
  /// Create a new distributed node that uses chitchat for service discovery
  /// and openraft for consensus, following Stract's architecture pattern
  pub async fn new(node_id: NodeId, db_path: String, options: Opt) -> Result<Self> {
    // 1. Create the openraft node (similar to existing implementation)
    let config = openraft::Config {
      heartbeat_interval: 250,
      election_timeout_min: 299,
      ..Default::default()
    };

    let config = Arc::new(config.validate().unwrap());

    let (log_store, state_machine_store) = crate::store::new_storage(&db_path).await;
    let kvs = state_machine_store.data.kvs.clone();
    let network = crate::network::Network {};

    let raft = openraft::Raft::new(
      node_id,
      config.clone(),
      network,
      log_store,
      state_machine_store,
    )
    .await?;

    // 2. Create chitchat cluster member for service discovery
    let service = ServiceType::RaftNode {
      api_addr: options.api_addr.clone(),
      rpc_addr: options.rpc_addr.clone(),
      raft_id: node_id,
    };

    let cluster_member = ClusterMember::new(service);

    // 3. Join the chitchat cluster
    let chitchat_gossip_addr: SocketAddr = options.gossip_addr.parse()?;
    let seed_addrs: Vec<SocketAddr> = options
      .seed_gossip_addrs
      .iter()
      .map(|addr| addr.parse())
      .collect::<Result<Vec<_>, _>>()?;

    let chitchat_cluster =
      ChitchatCluster::join(cluster_member, chitchat_gossip_addr, seed_addrs).await?;

    // 4. Create the API layer
    let api = Api {
      id: node_id,
      api_addr: options.api_addr.clone(),
      rpc_addr: options.rpc_addr.clone(),
      raft: raft.clone(),
      key_values: kvs,
      config,
    };

    Ok(Self {
      raft_node_id: node_id,
      raft,
      chitchat_cluster,
      api,
    })
  }

  /// Initialize a new raft cluster if this is the first node
  pub async fn initialize_raft_cluster(&self) -> Result<()> {
    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Discover other raft nodes through chitchat
    let raft_cluster = self.chitchat_cluster.discover_raft_cluster().await;

    tracing::info!("Discovered raft cluster: {:?}", raft_cluster);

    if raft_cluster.len() == 1 && raft_cluster.contains_key(&self.raft_node_id) {
      // We're the only node, initialize the cluster
      tracing::info!("Initializing new raft cluster as single node");
      self.raft.initialize(raft_cluster).await?;
    } else if !raft_cluster.is_empty() {
      // There are other nodes, try to join existing cluster
      tracing::info!("Attempting to join existing raft cluster");
      // Note: In a real implementation, you'd need to contact existing nodes
      // to be added as a learner first, then promoted to voting member
    }

    Ok(())
  }

  /// Join an existing raft cluster by contacting current members
  pub async fn join_raft_cluster(&self) -> Result<()> {
    // Wait for at least one other raft node to be available
    let raft_nodes = self.chitchat_cluster.await_raft_nodes(2).await;

    tracing::info!("Found {} raft nodes in cluster", raft_nodes.len());

    // Find a raft node that is not ourselves
    for member in raft_nodes {
      if let Some((api_addr, _rpc_addr, raft_id)) = member.get_raft_node_info() {
        if raft_id != self.raft_node_id {
          tracing::info!(
            "Attempting to join cluster through node {} at {}",
            raft_id,
            api_addr
          );

          // In a real implementation, you would:
          // 1. Contact the existing node's API
          // 2. Request to be added as a learner
          // 3. Wait for logs to sync
          // 4. Request promotion to voting member

          // For now, we'll just log the intent
          tracing::info!("Would contact {} to join cluster", api_addr);
          break;
        }
      }
    }

    Ok(())
  }

  /// Get current cluster topology from chitchat
  pub async fn get_cluster_topology(&self) -> BTreeMap<NodeId, Node> {
    self.chitchat_cluster.discover_raft_cluster().await
  }

  /// Update our service information in chitchat
  pub async fn update_service_info(&self, new_service: ServiceType) -> Result<()> {
    self.chitchat_cluster.set_service(new_service).await
  }

  /// Get all cluster members (not just raft nodes)
  pub async fn get_all_cluster_members(&self) -> Vec<ClusterMember> {
    self.chitchat_cluster.members().await
  }

  /// Wait for a minimum number of raft nodes to be available
  pub async fn wait_for_raft_quorum(&self, min_nodes: usize) -> Vec<ClusterMember> {
    tracing::info!(
      "Waiting for at least {} raft nodes to join cluster",
      min_nodes
    );
    self.chitchat_cluster.await_raft_nodes(min_nodes).await
  }
}

/// Stract-inspired startup function that combines chitchat service discovery
/// with openraft consensus
pub async fn start_distributed_raft_node(
  node_id: NodeId,
  db_path: String,
  options: Opt,
) -> Result<()> {
  tracing::info!(
    "Starting distributed raft node {} with chitchat service discovery",
    node_id
  );

  // Create the distributed node
  let distributed_node = DistributedNode::new(node_id, db_path, options.clone()).await?;

  // Wait a bit for chitchat to discover peers
  tokio::time::sleep(Duration::from_secs(1)).await;

  // Check if we should initialize or join
  let cluster_topology = distributed_node.get_cluster_topology().await;

  if cluster_topology.is_empty()
    || (cluster_topology.len() == 1 && cluster_topology.contains_key(&node_id))
  {
    // We're the first/only node, initialize
    distributed_node.initialize_raft_cluster().await?;
  } else {
    // Try to join existing cluster
    distributed_node.join_raft_cluster().await?;
  }

  // Start the API server
  let app = crate::web_openapi::create_api_service(distributed_node.api.clone()).await;

  let addr: SocketAddr = options.api_addr.parse()?;
  tracing::info!("Starting API server on {}", addr);

  poem::Server::new(poem::listener::TcpListener::bind(addr))
    .run(app)
    .await?;

  Ok(())
}

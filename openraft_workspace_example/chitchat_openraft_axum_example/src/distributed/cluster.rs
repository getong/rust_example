//! Cluster management using chitchat
//!
//! This module implements the Stract pattern where chitchat handles
//! cluster membership and service discovery.

use std::{collections::HashMap, net::SocketAddr, time::Duration};

use chitchat::{
  ChitchatConfig, ChitchatHandle, ChitchatId, FailureDetectorConfig, spawn_chitchat,
  transport::UdpTransport,
};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::distributed::member::{Member, MemberRegistry, Service, ShardId};

/// Cluster configuration
#[derive(Debug, Clone)]
pub struct ClusterConfig {
  pub chitchat_id: String,
  pub chitchat_listen_addr: SocketAddr,
  pub seed_nodes: Vec<SocketAddr>,
  pub heartbeat_interval: Duration,
  pub marked_for_deletion_grace_period: Duration,
}

impl Default for ClusterConfig {
  fn default() -> Self {
    Self {
      chitchat_id: "node-1".to_string(),
      chitchat_listen_addr: "127.0.0.1:10000".parse().unwrap(),
      seed_nodes: Vec::new(),
      heartbeat_interval: Duration::from_secs(1),
      marked_for_deletion_grace_period: Duration::from_secs(60),
    }
  }
}

/// Cluster manager that uses chitchat for membership
pub struct Cluster {
  config: ClusterConfig,
  chitchat_handle: Option<ChitchatHandle>,
  members: MemberRegistry,
  local_service: Service,
}

impl Cluster {
  /// Create a new cluster manager
  pub fn new(config: ClusterConfig, local_service: Service) -> Self {
    Self {
      config,
      chitchat_handle: None,
      members: HashMap::new(),
      local_service,
    }
  }

  /// Start the cluster (spawn chitchat)
  pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting cluster with service: {}", self.local_service);

    let generation = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_secs();

    let chitchat_id = ChitchatId::new(
      self.config.chitchat_id.clone(),
      generation,
      self.config.chitchat_listen_addr,
    );

    let chitchat_config = ChitchatConfig {
      cluster_id: "chitchat-openraft-cluster".to_string(),
      chitchat_id,
      gossip_interval: Duration::from_secs(1),
      listen_addr: self.config.chitchat_listen_addr,
      seed_nodes: self
        .config
        .seed_nodes
        .iter()
        .map(|addr| addr.to_string())
        .collect(),
      failure_detector_config: FailureDetectorConfig {
        dead_node_grace_period: Duration::from_secs(10),
        ..FailureDetectorConfig::default()
      },
      marked_for_deletion_grace_period: self.config.marked_for_deletion_grace_period,
      catchup_callback: None,
      extra_liveness_predicate: None,
    };

    let chitchat_handle = spawn_chitchat(chitchat_config, Vec::new(), &UdpTransport).await?;

    // Register our service in chitchat
    self
      .register_service(&chitchat_handle, &self.local_service)
      .await?;

    self.chitchat_handle = Some(chitchat_handle);

    info!("Cluster started successfully");
    Ok(())
  }

  /// Register a service with chitchat
  async fn register_service(
    &self,
    chitchat_handle: &ChitchatHandle,
    service: &Service,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let service_data = serde_json::to_string(service)?;

    let chitchat = chitchat_handle.chitchat();
    let mut chitchat_guard = chitchat.lock().await;
    let cc_state = chitchat_guard.self_node_state();

    cc_state.set("service", &service_data);
    cc_state.set("ready", "false");

    debug!("Registered service: {}", service);
    Ok(())
  }

  /// Mark our service as ready
  pub async fn mark_ready(&self) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(ref handle) = self.chitchat_handle {
      let chitchat = handle.chitchat();
      let mut chitchat_guard = chitchat.lock().await;
      let cc_state = chitchat_guard.self_node_state();
      cc_state.set("ready", "true");
      info!("Service marked as ready");
    }
    Ok(())
  }

  /// Update member registry from chitchat
  pub async fn update_members(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let Some(ref handle) = self.chitchat_handle else {
      return Ok(());
    };

    let chitchat = handle.chitchat();
    let chitchat_guard = chitchat.lock().await;

    let mut new_members = HashMap::new();

    // For now, just add ourselves to the member list
    // In a full implementation, you would iterate through all cluster nodes
    // and extract their service information

    let node_id = 1u32; // Use a hash or counter for unique IDs
    let member = Member::new(node_id, self.local_service.clone());
    new_members.insert(node_id, member);

    self.members = new_members;
    debug!("Updated member registry: {} members", self.members.len());
    Ok(())
  }

  /// Get all members
  pub fn members(&self) -> &MemberRegistry {
    &self.members
  }

  /// Get DHT members for a specific shard
  pub fn get_dht_shard_members(&self, shard: ShardId) -> Vec<&Member> {
    use crate::distributed::member::helpers::get_dht_shard_members;
    get_dht_shard_members(&self.members, shard)
  }

  /// Get all DHT members
  pub fn get_dht_members(&self) -> Vec<&Member> {
    use crate::distributed::member::helpers::get_dht_members;
    get_dht_members(&self.members)
  }

  /// Get API members
  pub fn get_api_members(&self) -> Vec<&Member> {
    use crate::distributed::member::helpers::get_api_members;
    get_api_members(&self.members)
  }

  /// Get ready members only
  pub fn get_ready_members(&self) -> Vec<&Member> {
    use crate::distributed::member::helpers::get_ready_members;
    get_ready_members(&self.members)
  }

  /// Run the cluster membership loop
  pub async fn run_membership_loop(&mut self) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));

    loop {
      interval.tick().await;

      if let Err(e) = self.update_members().await {
        error!("Failed to update members: {}", e);
      }
    }
  }

  /// Stop the cluster
  pub async fn stop(&mut self) {
    if let Some(handle) = self.chitchat_handle.take() {
      if let Err(e) = handle.shutdown().await {
        error!("Failed to shutdown chitchat: {}", e);
      }
    }
    info!("Cluster stopped");
  }
}

impl Drop for Cluster {
  fn drop(&mut self) {
    if self.chitchat_handle.is_some() {
      warn!("Cluster dropped without proper shutdown");
    }
  }
}

#[cfg(test)]
mod tests {
  use std::net::{IpAddr, Ipv4Addr};

  use super::*;

  #[tokio::test]
  async fn test_cluster_creation() {
    let config = ClusterConfig::default();
    let service = Service::Api {
      host: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
    };

    let cluster = Cluster::new(config, service);
    assert!(cluster.chitchat_handle.is_none());
    assert_eq!(cluster.members.len(), 0);
  }

  #[test]
  fn test_cluster_config_default() {
    let config = ClusterConfig::default();
    assert_eq!(config.chitchat_id, "node-1");
    assert_eq!(config.seed_nodes.len(), 0);
  }
}

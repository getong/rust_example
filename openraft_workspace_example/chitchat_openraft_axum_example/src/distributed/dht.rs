//! Distributed Hash Table (DHT) using OpenRaft
//!
//! This module implements a consistent DHT using OpenRaft for consensus,
//! based on the Stract pattern. Each DHT shard is a separate Raft cluster.
//!
//! Note: This is a simplified implementation focused on demonstrating the
//! chitchat + openraft integration pattern. Full OpenRaft traits would be
//! implemented in a production system.

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::distributed::member::{NodeId, ShardId};

/// DHT request types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DhtRequest {
  Put { key: String, value: String },
  Get { key: String },
  Delete { key: String },
}

/// DHT response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DhtResponse {
  PutResponse,
  GetResponse { value: Option<String> },
  DeleteResponse { existed: bool },
}

/// DHT state machine data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DhtStateMachineData {
  pub data: HashMap<String, String>,
  pub last_applied_log: Option<u64>,
}

/// DHT state machine
pub struct DhtStateMachine {
  data: Arc<RwLock<DhtStateMachineData>>,
}

impl DhtStateMachine {
  pub fn new() -> Self {
    Self {
      data: Arc::new(RwLock::new(DhtStateMachineData::default())),
    }
  }

  pub async fn get(&self, key: &str) -> Option<String> {
    let data = self.data.read().await;
    data.data.get(key).cloned()
  }

  pub async fn put(&self, key: String, value: String) {
    let mut data = self.data.write().await;
    data.data.insert(key, value);
  }

  pub async fn delete(&self, key: &str) -> bool {
    let mut data = self.data.write().await;
    data.data.remove(key).is_some()
  }

  pub async fn len(&self) -> usize {
    let data = self.data.read().await;
    data.data.len()
  }
}

impl Default for DhtStateMachine {
  fn default() -> Self {
    Self::new()
  }
}

/// DHT log storage (simplified in-memory implementation)
/// In a production system, this would implement the full OpenRaft storage traits
pub struct DhtLogStorage {
  logs: Arc<RwLock<HashMap<u64, String>>>,
  current_term: Arc<RwLock<u64>>,
  voted_for: Arc<RwLock<Option<NodeId>>>,
}

impl DhtLogStorage {
  pub fn new() -> Self {
    Self {
      logs: Arc::new(RwLock::new(HashMap::new())),
      current_term: Arc::new(RwLock::new(0)),
      voted_for: Arc::new(RwLock::new(None)),
    }
  }

  pub async fn get_current_term(&self) -> u64 {
    *self.current_term.read().await
  }

  pub async fn set_current_term(&self, term: u64) {
    *self.current_term.write().await = term;
  }

  pub async fn get_voted_for(&self) -> Option<NodeId> {
    *self.voted_for.read().await
  }

  pub async fn set_voted_for(&self, node_id: Option<NodeId>) {
    *self.voted_for.write().await = node_id;
  }
}

impl Default for DhtLogStorage {
  fn default() -> Self {
    Self::new()
  }
}

/// DHT network layer for Raft communication
/// In a production system, this would implement the full OpenRaft network traits
pub struct DhtNetwork {
  peers: Arc<RwLock<HashMap<NodeId, SocketAddr>>>,
  client: reqwest::Client,
}

impl DhtNetwork {
  pub fn new() -> Self {
    Self {
      peers: Arc::new(RwLock::new(HashMap::new())),
      client: reqwest::Client::new(),
    }
  }

  pub async fn add_peer(&self, node_id: NodeId, addr: SocketAddr) {
    let mut peers = self.peers.write().await;
    peers.insert(node_id, addr);
  }

  pub async fn remove_peer(&self, node_id: NodeId) {
    let mut peers = self.peers.write().await;
    peers.remove(&node_id);
  }

  pub async fn get_peers(&self) -> HashMap<NodeId, SocketAddr> {
    self.peers.read().await.clone()
  }
}

impl Default for DhtNetwork {
  fn default() -> Self {
    Self::new()
  }
}

/// DHT server that manages a single shard
/// In a production system, this would integrate with a full OpenRaft instance
pub struct DhtServer {
  node_id: NodeId,
  shard_id: ShardId,
  listen_addr: SocketAddr,
  state_machine: DhtStateMachine,
  network: DhtNetwork,
  log_storage: DhtLogStorage,
  is_leader: Arc<RwLock<bool>>,
}

impl DhtServer {
  pub fn new(node_id: NodeId, shard_id: ShardId, listen_addr: SocketAddr) -> Self {
    Self {
      node_id,
      shard_id,
      listen_addr,
      state_machine: DhtStateMachine::new(),
      network: DhtNetwork::new(),
      log_storage: DhtLogStorage::new(),
      is_leader: Arc::new(RwLock::new(true)), // Simplified: assume leader for demo
    }
  }

  /// Start the DHT server
  pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    info!(
      "Starting DHT server node {} for shard {}",
      self.node_id, self.shard_id
    );

    // In a production system, you would:
    // 1. Initialize OpenRaft with proper storage and network implementations
    // 2. Join the Raft cluster or start as a single-node cluster
    // 3. Set up HTTP/gRPC servers for client requests
    // 4. Implement proper consensus for all operations

    info!(
      "DHT server started on {} (simplified implementation)",
      self.listen_addr
    );
    Ok(())
  }

  /// Check if this node is the leader
  pub async fn is_leader(&self) -> bool {
    *self.is_leader.read().await
  }

  /// Set leadership status (simplified for demo)
  pub async fn set_leader(&self, is_leader: bool) {
    *self.is_leader.write().await = is_leader;
  }

  /// Handle DHT requests
  pub async fn handle_request(
    &self,
    request: DhtRequest,
  ) -> Result<DhtResponse, Box<dyn std::error::Error>> {
    // In a production system, only the leader should handle writes
    // and reads could be handled by followers with appropriate consistency guarantees

    match request {
      DhtRequest::Put { key, value } => {
        if !self.is_leader().await {
          return Err("Not the leader, cannot handle write request".into());
        }

        // In production: this would go through Raft consensus
        self.state_machine.put(key, value).await;
        debug!("PUT operation completed");
        Ok(DhtResponse::PutResponse)
      }
      DhtRequest::Get { key } => {
        // Reads can be handled by any node, but may not be linearizable
        // In production: you'd implement read-only queries or forward to leader
        let value = self.state_machine.get(&key).await;
        debug!("GET operation completed for key: {}", key);
        Ok(DhtResponse::GetResponse { value })
      }
      DhtRequest::Delete { key } => {
        if !self.is_leader().await {
          return Err("Not the leader, cannot handle write request".into());
        }

        // In production: this would go through Raft consensus
        let existed = self.state_machine.delete(&key).await;
        debug!("DELETE operation completed for key: {}", key);
        Ok(DhtResponse::DeleteResponse { existed })
      }
    }
  }

  /// Add a peer to the Raft cluster
  pub async fn add_peer(&self, node_id: NodeId, addr: SocketAddr) {
    self.network.add_peer(node_id, addr).await;
  }

  /// Remove a peer from the Raft cluster
  pub async fn remove_peer(&self, node_id: NodeId) {
    self.network.remove_peer(node_id).await;
  }

  /// Get the current state of the DHT
  pub async fn get_state(&self) -> HashMap<String, String> {
    let data = self.state_machine.data.read().await;
    data.data.clone()
  }

  /// Get the size of the DHT
  pub async fn size(&self) -> usize {
    self.state_machine.len().await
  }

  /// Get node info
  pub fn node_id(&self) -> NodeId {
    self.node_id
  }

  pub fn shard_id(&self) -> ShardId {
    self.shard_id
  }

  pub fn listen_addr(&self) -> SocketAddr {
    self.listen_addr
  }

  /// Get the current Raft term (simplified)
  pub async fn get_current_term(&self) -> u64 {
    self.log_storage.get_current_term().await
  }

  /// Get the peers in this Raft cluster
  pub async fn get_peers(&self) -> HashMap<NodeId, SocketAddr> {
    self.network.get_peers().await
  }
}

/// DHT client for interacting with the distributed hash table
pub struct DhtClient {
  servers: HashMap<ShardId, Vec<SocketAddr>>,
  client: reqwest::Client,
}

impl DhtClient {
  pub fn new() -> Self {
    Self {
      servers: HashMap::new(),
      client: reqwest::Client::new(),
    }
  }

  /// Add servers for a shard
  pub fn add_shard_servers(&mut self, shard_id: ShardId, servers: Vec<SocketAddr>) {
    self.servers.insert(shard_id, servers);
  }

  /// Calculate which shard a key belongs to
  pub fn calculate_shard(&self, key: &str) -> ShardId {
    use std::{
      collections::hash_map::DefaultHasher,
      hash::{Hash, Hasher},
    };

    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let hash = hasher.finish();

    let num_shards = self.servers.len() as u64;
    if num_shards == 0 {
      return 0;
    }

    (hash % num_shards) as ShardId
  }

  /// Put a key-value pair
  pub async fn put(&self, key: String, value: String) -> Result<(), Box<dyn std::error::Error>> {
    let shard_id = self.calculate_shard(&key);

    if let Some(servers) = self.servers.get(&shard_id) {
      if let Some(server) = servers.first() {
        let request = DhtRequest::Put { key, value };
        // In a real implementation, you'd send this via HTTP/gRPC
        debug!(
          "Would send PUT request to shard {} server {}",
          shard_id, server
        );
      }
    }

    Ok(())
  }

  /// Get a value by key
  pub async fn get(&self, key: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let shard_id = self.calculate_shard(key);

    if let Some(servers) = self.servers.get(&shard_id) {
      if let Some(server) = servers.first() {
        let request = DhtRequest::Get {
          key: key.to_string(),
        };
        // In a real implementation, you'd send this via HTTP/gRPC
        debug!(
          "Would send GET request to shard {} server {}",
          shard_id, server
        );
      }
    }

    Ok(None)
  }

  /// Delete a key
  pub async fn delete(&self, key: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let shard_id = self.calculate_shard(key);

    if let Some(servers) = self.servers.get(&shard_id) {
      if let Some(server) = servers.first() {
        let request = DhtRequest::Delete {
          key: key.to_string(),
        };
        // In a real implementation, you'd send this via HTTP/gRPC
        debug!(
          "Would send DELETE request to shard {} server {}",
          shard_id, server
        );
      }
    }

    Ok(false)
  }
}

impl Default for DhtClient {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use std::net::{IpAddr, Ipv4Addr};

  use super::*;

  #[test]
  fn test_dht_state_machine_creation() {
    let sm = DhtStateMachine::new();
    // Test that it's created successfully
    assert!(sm.data.try_read().is_ok());
  }

  #[tokio::test]
  async fn test_dht_state_machine_operations() {
    let sm = DhtStateMachine::new();

    // Test put and get
    sm.put("key1".to_string(), "value1".to_string()).await;
    let value = sm.get("key1").await;
    assert_eq!(value, Some("value1".to_string()));

    // Test delete
    let existed = sm.delete("key1").await;
    assert!(existed);

    let value = sm.get("key1").await;
    assert_eq!(value, None);
  }

  #[test]
  fn test_dht_server_creation() {
    let server = DhtServer::new(
      1,
      0,
      SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
    );
    assert_eq!(server.node_id(), 1);
    assert_eq!(server.shard_id(), 0);
  }

  #[test]
  fn test_dht_client_shard_calculation() {
    let mut client = DhtClient::new();
    client.add_shard_servers(
      0,
      vec![SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        8080,
      )],
    );
    client.add_shard_servers(
      1,
      vec![SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        8081,
      )],
    );

    let shard1 = client.calculate_shard("key1");
    let shard2 = client.calculate_shard("key2");

    // Same key should always map to the same shard
    assert_eq!(shard1, client.calculate_shard("key1"));

    // Different keys might map to different shards
    assert!(shard1 < 2);
    assert!(shard2 < 2);
  }

  #[tokio::test]
  async fn test_dht_request_handling() {
    let server = DhtServer::new(
      1,
      0,
      SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
    );

    // Test put request
    let put_request = DhtRequest::Put {
      key: "test_key".to_string(),
      value: "test_value".to_string(),
    };
    let response = server.handle_request(put_request).await.unwrap();
    assert!(matches!(response, DhtResponse::PutResponse));

    // Test get request
    let get_request = DhtRequest::Get {
      key: "test_key".to_string(),
    };
    let response = server.handle_request(get_request).await.unwrap();
    if let DhtResponse::GetResponse { value } = response {
      assert_eq!(value, Some("test_value".to_string()));
    } else {
      panic!("Expected GetResponse");
    }
  }
}

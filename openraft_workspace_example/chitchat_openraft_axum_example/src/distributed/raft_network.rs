//! Network implementation for OpenRaft based on Stract's pattern
//!
//! This provides the production-ready RaftNetwork trait implementation
//! for peer-to-peer communication between Raft nodes.

use std::collections::BTreeSet;
use std::sync::Arc;

use openraft::error::{NetworkError, RPCError, RemoteError};
use openraft::network::{RPCOption, RaftNetwork, RaftNetworkFactory};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use openraft::{BasicNode, RaftTypeConfig};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::raft_types::{DhtTypeConfig, BasicNode as DhtBasicNode};
use crate::distributed::member::NodeId;

/// Network client for communicating with remote Raft nodes
#[derive(Clone)]
pub struct DhtNetworkClient {
    node_id: NodeId,
    target_addr: String,
    client: reqwest::Client,
}

impl DhtNetworkClient {
    pub fn new(node_id: NodeId, target_addr: String) -> Self {
        Self {
            node_id,
            target_addr,
            client: reqwest::Client::new(),
        }
    }

    async fn send_request<Req, Resp>(
        &self,
        endpoint: &str,
        request: &Req,
    ) -> Result<Resp, RPCError<NodeId, openraft::error::NetworkError>>
    where
        Req: serde::Serialize,
        Resp: serde::de::DeserializeOwned,
    {
        let url = format!("http://{}/{}", self.target_addr, endpoint);
        
        debug!(
            "Sending {} request to node {} at {}",
            endpoint, self.node_id, url
        );

        let response = self
            .client
            .post(&url)
            .json(request)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send request to {}: {}", url, e);
                RPCError::Network(NetworkError::new(&e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "Received error response from {}: {} - {}",
                url, status, error_text
            );
            return Err(RPCError::Network(NetworkError::new(&format!(
                "HTTP {}: {}",
                status, error_text
            ))));
        }

        let result = response.json().await.map_err(|e| {
            error!("Failed to parse response from {}: {}", url, e);
            RPCError::Network(NetworkError::new(&e))
        })?;

        debug!("Successfully received response from {}", url);
        Ok(result)
    }
}

#[async_trait::async_trait]
impl RaftNetwork<DhtTypeConfig> for DhtNetworkClient {
    async fn vote(
        &mut self,
        rpc: VoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, NetworkError>> {
        debug!("Sending vote request to node {}", self.node_id);
        self.send_request("raft/vote", &rpc).await
    }

    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<DhtTypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, NetworkError>> {
        debug!("Sending append_entries request to node {}", self.node_id);
        self.send_request("raft/append_entries", &rpc).await
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<DhtTypeConfig>,
        _option: RPCOption,
    ) -> Result<InstallSnapshotResponse<NodeId>, RPCError<NodeId, NetworkError>> {
        debug!("Sending install_snapshot request to node {}", self.node_id);
        self.send_request("raft/install_snapshot", &rpc).await
    }
}

/// Network factory for creating network clients
pub struct DhtNetworkFactory {
    /// Mapping of node IDs to their network addresses
    node_addresses: Arc<RwLock<std::collections::HashMap<NodeId, String>>>,
}

impl DhtNetworkFactory {
    pub fn new() -> Self {
        Self {
            node_addresses: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Add or update a node's address
    pub async fn add_node(&self, node_id: NodeId, address: String) {
        let mut addresses = self.node_addresses.write().await;
        addresses.insert(node_id, address);
        info!("Added/updated node {} with address", node_id);
    }

    /// Remove a node's address
    pub async fn remove_node(&self, node_id: NodeId) {
        let mut addresses = self.node_addresses.write().await;
        if addresses.remove(&node_id).is_some() {
            info!("Removed node {}", node_id);
        }
    }

    /// Get all known node addresses
    pub async fn get_all_nodes(&self) -> std::collections::HashMap<NodeId, String> {
        let addresses = self.node_addresses.read().await;
        addresses.clone()
    }
}

impl Default for DhtNetworkFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DhtNetworkFactory {
    fn clone(&self) -> Self {
        Self {
            node_addresses: self.node_addresses.clone(),
        }
    }
}

#[async_trait::async_trait]
impl RaftNetworkFactory<DhtTypeConfig> for DhtNetworkFactory {
    type Network = DhtNetworkClient;

    async fn new_client(&mut self, target: NodeId, node: &DhtBasicNode) -> Self::Network {
        info!("Creating new network client for node {} at {}", target, node.addr);
        
        // Update our address mapping
        self.add_node(target, node.addr.clone()).await;
        
        DhtNetworkClient::new(target, node.addr.clone())
    }
}

/// Simple network implementation for testing and development
pub struct SimpleNetwork;

#[async_trait::async_trait]
impl RaftNetwork<DhtTypeConfig> for SimpleNetwork {
    async fn vote(
        &mut self,
        _rpc: VoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, NetworkError>> {
        warn!("SimpleNetwork: vote request - returning success for demo");
        Ok(VoteResponse {
            vote: openraft::Vote::new(1, 1),
            vote_granted: true,
            last_log_id: None,
        })
    }

    async fn append_entries(
        &mut self,
        _rpc: AppendEntriesRequest<DhtTypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, NetworkError>> {
        warn!("SimpleNetwork: append_entries request - returning success for demo");
        Ok(AppendEntriesResponse {
            vote: openraft::Vote::new(1, 1),
            success: true,
            conflict: None,
        })
    }

    async fn install_snapshot(
        &mut self,
        _rpc: InstallSnapshotRequest<DhtTypeConfig>,
        _option: RPCOption,
    ) -> Result<InstallSnapshotResponse<NodeId>, RPCError<NodeId, NetworkError>> {
        warn!("SimpleNetwork: install_snapshot request - returning success for demo");
        Ok(InstallSnapshotResponse {
            vote: openraft::Vote::new(1, 1),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_factory() {
        let factory = DhtNetworkFactory::new();
        
        // Add some nodes
        factory.add_node(1, "127.0.0.1:8001".to_string()).await;
        factory.add_node(2, "127.0.0.1:8002".to_string()).await;
        
        // Verify they were added
        let nodes = factory.get_all_nodes().await;
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes.get(&1), Some(&"127.0.0.1:8001".to_string()));
        assert_eq!(nodes.get(&2), Some(&"127.0.0.1:8002".to_string()));
        
        // Remove a node
        factory.remove_node(1).await;
        let nodes = factory.get_all_nodes().await;
        assert_eq!(nodes.len(), 1);
        assert!(!nodes.contains_key(&1));
    }

    #[tokio::test]
    async fn test_network_client_creation() {
        let mut factory = DhtNetworkFactory::new();
        let node = DhtBasicNode::new("127.0.0.1:8080");
        
        let client = factory.new_client(1, &node).await;
        assert_eq!(client.node_id, 1);
        assert_eq!(client.target_addr, "127.0.0.1:8080");
        
        // Verify the address was stored
        let nodes = factory.get_all_nodes().await;
        assert_eq!(nodes.get(&1), Some(&"127.0.0.1:8080".to_string()));
    }
}

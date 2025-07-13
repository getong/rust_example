//! OpenRaft Integration Module
//! 
//! Handles distributed consensus and consistency using OpenRaft.
//! This provides the strong consistency guarantees needed for distributed systems.

use crate::distributed::{
    raft_types::{NodeId, DhtTypeConfig, DhtRequest, DhtResponse, BasicNode},
    raft_state_machine::{StateMachine, StateMachineResponse},
    raft_log_storage::LogStorage,
    raft_network::RaftNetwork,
};

use openraft::{
    Config, Raft, RaftMetrics, LogId, EntryPayload,
    storage::{Adaptor, RaftLogStorage, RaftStateMachine},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

/// Configuration for OpenRaft
#[derive(Debug, Clone)]
pub struct RaftConfig {
    pub node_id: NodeId,
    pub cluster_name: String,
    pub election_timeout_min: u64,
    pub election_timeout_max: u64,
    pub heartbeat_interval: u64,
    pub install_snapshot_timeout: u64,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            node_id: 1,
            cluster_name: "stract-cluster".to_string(),
            election_timeout_min: 150,
            election_timeout_max: 300,
            heartbeat_interval: 50,
            install_snapshot_timeout: 1000,
        }
    }
}

/// Request types for the Raft state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftRequest {
    /// Set a key-value pair
    Set { key: String, value: String },
    /// Get a value by key
    Get { key: String },
    /// Delete a key
    Delete { key: String },
    /// Increment a counter
    Increment { key: String },
    /// Custom application-specific request
    Custom { operation: String, data: serde_json::Value },
}

impl From<RaftRequest> for DhtRequest {
    fn from(req: RaftRequest) -> Self {
        match req {
            RaftRequest::Set { key, value } => DhtRequest::Put { key, value },
            RaftRequest::Get { key } => DhtRequest::Get { key },
            RaftRequest::Delete { key } => DhtRequest::Delete { key },
            RaftRequest::Increment { key } => DhtRequest::Put { key: key.clone(), value: "1".to_string() },
            RaftRequest::Custom { .. } => DhtRequest::Get { key: "custom".to_string() },
        }
    }
}

/// Response types from the Raft state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftResponse {
    /// Success response with optional data
    Success { data: Option<String> },
    /// Error response
    Error { message: String },
    /// Value response for get operations
    Value { key: String, value: Option<String> },
    /// Counter response
    Counter { key: String, value: u64 },
}

impl From<DhtResponse> for RaftResponse {
    fn from(resp: DhtResponse) -> Self {
        match resp {
            DhtResponse::Put => RaftResponse::Success { data: None },
            DhtResponse::Get { value } => RaftResponse::Value { 
                key: "unknown".to_string(), 
                value 
            },
            DhtResponse::Delete { existed } => RaftResponse::Success { 
                data: Some(existed.to_string()) 
            },
            DhtResponse::BatchPut => RaftResponse::Success { data: None },
            DhtResponse::BatchDelete { deleted_count } => RaftResponse::Success { 
                data: Some(deleted_count.to_string()) 
            },
            DhtResponse::Empty => RaftResponse::Success { data: None },
        }
    }
}

/// Manages OpenRaft consensus and consistency
pub struct RaftManager {
    raft: Option<Raft<DhtTypeConfig>>,
    config: RaftConfig,
    storage: Arc<LogStorage>,
    state_machine: Arc<StateMachine>,
    network: Arc<RaftNetwork>,
    metrics: RwLock<Option<RaftMetrics<NodeId, BasicNode>>>,
}

impl RaftManager {
    /// Create a new raft manager
    pub fn new(config: RaftConfig) -> Self {
        let storage = Arc::new(LogStorage::new());
        let state_machine = Arc::new(StateMachine::new());
        let network = Arc::new(RaftNetwork::new());

        Self {
            raft: None,
            config,
            storage,
            state_machine,
            network,
            metrics: RwLock::new(None),
        }
    }

    /// Initialize and start the Raft node
    pub async fn start(&mut self, initial_members: Vec<NodeId>) -> anyhow::Result<()> {
        info!("🚀 Starting OpenRaft consensus service for node {}", self.config.node_id);

        // Create OpenRaft config
        let raft_config = Config {
            heartbeat_interval: self.config.heartbeat_interval,
            election_timeout_min: self.config.election_timeout_min,
            election_timeout_max: self.config.election_timeout_max,
            install_snapshot_timeout: self.config.install_snapshot_timeout,
            cluster_name: self.config.cluster_name.clone(),
            ..Default::default()
        };

        // Create storage adaptor
        let log_storage = Adaptor::new(self.storage.clone());
        let state_machine = Adaptor::new(self.state_machine.clone());

        // Create Raft instance
        let raft = Raft::new(
            self.config.node_id,
            raft_config.into(),
            self.network.clone(),
            log_storage,
            state_machine,
        ).await?;

        // Initialize cluster if this is the first node
        if initial_members.len() == 1 && initial_members[0] == self.config.node_id {
            info!("🌱 Initializing new Raft cluster");
            let mut nodes = std::collections::BTreeSet::new();
            nodes.insert(self.config.node_id);
            raft.initialize(nodes).await?;
            info!("✅ Raft cluster initialized successfully");
        }

        // Store the raft instance
        self.raft = Some(raft.clone());

        // Start background task to monitor metrics
        let metrics_clone = self.metrics.clone();
        tokio::spawn(async move {
            Self::metrics_monitor(raft.clone(), metrics_clone).await;
        });

        info!("✅ OpenRaft consensus service started successfully");
        Ok(())
    }

    /// Background task to monitor Raft metrics
    async fn metrics_monitor(
        raft: Raft<DhtTypeConfig>,
        metrics: RwLock<Option<RaftMetrics<NodeId, BasicNode>>>,
    ) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        
        loop {
            interval.tick().await;
            
            if let Ok(current_metrics) = raft.metrics().await {
                let is_leader = current_metrics.current_leader == Some(raft.id());
                let term = current_metrics.current_term;
                let last_log_index = current_metrics.last_log_index;
                
                debug!("📊 Raft metrics: leader={}, term={}, last_log={:?}", 
                       is_leader, term, last_log_index);
                
                *metrics.write().await = Some(current_metrics);
            }
        }
    }

    /// Submit a request to the Raft cluster
    pub async fn submit_request(&self, request: RaftRequest) -> anyhow::Result<RaftResponse> {
        let raft = self.raft.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Raft not initialized"))?;

        debug!("📤 Submitting Raft request: {:?}", request);

        // Convert to DHT request
        let dht_request: DhtRequest = request.into();
        
        // Submit to Raft
        let response = raft.client_write(dht_request).await?;
        
        // Convert response back
        let raft_response: RaftResponse = response.data.into();
        
        debug!("📥 Received Raft response: {:?}", raft_response);
        Ok(raft_response)
    }

    /// Read from the state machine (consistent read)
    pub async fn consistent_read(&self, request: RaftRequest) -> anyhow::Result<RaftResponse> {
        let raft = self.raft.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Raft not initialized"))?;

        debug!("📖 Performing consistent read: {:?}", request);

        // For reads, we need to ensure we're reading from the leader
        // This ensures linearizable consistency
        let dht_request: DhtRequest = request.into();
        let response = raft.client_write(dht_request).await?;
        let raft_response: RaftResponse = response.data.into();
        
        debug!("📚 Consistent read response: {:?}", raft_response);
        Ok(raft_response)
    }

    /// Add a new node to the cluster
    pub async fn add_node(&self, node_id: NodeId, addr: String) -> anyhow::Result<()> {
        let raft = self.raft.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Raft not initialized"))?;

        info!("➕ Adding node {} to Raft cluster", node_id);

        // Create node definition
        let node = BasicNode::new(&addr);
        
        // Add the node to cluster
        raft.add_learner(node_id, node, true).await?;
        raft.change_membership([node_id], false).await?;
        
        info!("✅ Successfully added node {} to cluster", node_id);
        Ok(())
    }

    /// Remove a node from the cluster
    pub async fn remove_node(&self, node_id: NodeId) -> anyhow::Result<()> {
        let raft = self.raft.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Raft not initialized"))?;

        info!("➖ Removing node {} from Raft cluster", node_id);

        // Get current membership and remove the node
        let metrics = raft.metrics().await?;
        if let Some(membership) = metrics.membership_config.membership() {
            let mut new_members: Vec<NodeId> = membership.voter_ids().collect();
            new_members.retain(|&id| id != node_id);
            
            raft.change_membership(new_members, false).await?;
        }
        
        info!("✅ Successfully removed node {} from cluster", node_id);
        Ok(())
    }

    /// Check if this node is the leader
    pub async fn is_leader(&self) -> bool {
        if let Some(raft) = &self.raft {
            if let Ok(metrics) = raft.metrics().await {
                return metrics.current_leader == Some(self.config.node_id);
            }
        }
        false
    }

    /// Get current leader node ID
    pub async fn get_leader(&self) -> Option<NodeId> {
        if let Some(raft) = &self.raft {
            if let Ok(metrics) = raft.metrics().await {
                return metrics.current_leader;
            }
        }
        None
    }

    /// Get cluster metrics
    pub async fn get_metrics(&self) -> Option<RaftMetrics<NodeId, BasicNode>> {
        self.metrics.read().await.clone()
    }

    /// Wait for leadership
    pub async fn wait_for_leadership(&self) -> anyhow::Result<()> {
        let raft = self.raft.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Raft not initialized"))?;

        info!("⏳ Waiting for leadership election...");

        // Wait until this node becomes leader or finds a leader
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
        let timeout = std::time::Duration::from_secs(30);
        let start = std::time::Instant::now();

        loop {
            interval.tick().await;

            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("Timeout waiting for leadership"));
            }

            if let Ok(metrics) = raft.metrics().await {
                if metrics.current_leader.is_some() {
                    if metrics.current_leader == Some(self.config.node_id) {
                        info!("👑 Node {} became leader", self.config.node_id);
                    } else {
                        info!("👥 Found leader: {:?}", metrics.current_leader);
                    }
                    return Ok(());
                }
            }
        }
    }

    /// Get the Raft instance for direct access
    pub fn raft(&self) -> Option<&Raft<DhtTypeConfig>> {
        self.raft.as_ref()
    }

    /// Shutdown the Raft node
    pub async fn shutdown(&mut self) {
        if let Some(raft) = &self.raft {
            info!("🛑 Shutting down OpenRaft consensus service");
            let _ = raft.shutdown().await;
        }
        self.raft = None;
    }
}

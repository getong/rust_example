//! Cluster Coordinator Module
//! 
//! High-level coordinator that manages both chitchat and OpenRaft integration.
//! This provides a unified interface for cluster operations.

use crate::core::{
    chitchat_integration::{ChitchatManager, ChitchatConfig, ServiceType},
    openraft_integration::{RaftManager, RaftConfig, RaftRequest, RaftResponse},
    service_registry::{ServiceRegistry, ServiceInfo, ClusterSummary},
};

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

/// Configuration for the cluster coordinator
#[derive(Debug, Clone)]
pub struct ClusterConfig {
    pub chitchat_config: ChitchatConfig,
    pub raft_config: RaftConfig,
    pub service_type: ServiceType,
    pub service_addr: Option<SocketAddr>,
}

/// High-level cluster coordinator that manages distributed system components
pub struct ClusterCoordinator {
    chitchat: Arc<RwLock<ChitchatManager>>,
    raft: Arc<RwLock<RaftManager>>,
    service_registry: Arc<ServiceRegistry>,
    config: ClusterConfig,
    is_running: RwLock<bool>,
}

impl ClusterCoordinator {
    /// Create a new cluster coordinator
    pub fn new(config: ClusterConfig) -> Self {
        let mut chitchat = ChitchatManager::new(
            config.chitchat_config.clone(),
            config.service_type.clone(),
        );

        if let Some(service_addr) = config.service_addr {
            chitchat.set_service_addr(service_addr);
        }

        let chitchat = Arc::new(RwLock::new(chitchat));
        let raft = Arc::new(RwLock::new(RaftManager::new(config.raft_config.clone())));
        let service_registry = Arc::new(ServiceRegistry::new(chitchat.clone()));

        Self {
            chitchat,
            raft,
            service_registry,
            config,
            is_running: RwLock::new(false),
        }
    }

    /// Start the cluster coordinator
    pub async fn start(&self) -> anyhow::Result<()> {
        info!("🚀 Starting cluster coordinator for node {}", self.config.raft_config.node_id);

        // Check if already running
        if *self.is_running.read().await {
            return Err(anyhow::anyhow!("Cluster coordinator already running"));
        }

        // Start chitchat first
        {
            let mut chitchat = self.chitchat.write().await;
            chitchat.start().await?;
        }

        // Wait a bit for chitchat to stabilize
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Start raft with initial members (for now, just this node)
        {
            let mut raft = self.raft.write().await;
            raft.start(vec![self.config.raft_config.node_id]).await?;
        }

        // Start service registry
        self.service_registry.start().await;

        // Mark as running
        *self.is_running.write().await = true;

        info!("✅ Cluster coordinator started successfully");
        Ok(())
    }

    /// Join an existing cluster
    pub async fn join_cluster(&self, seed_nodes: Vec<SocketAddr>) -> anyhow::Result<()> {
        info!("🤝 Joining cluster via seeds: {:?}", seed_nodes);

        // Update chitchat config with seed nodes
        {
            let mut chitchat = self.chitchat.write().await;
            chitchat.config.seed_nodes = seed_nodes;
        }

        // Start the coordinator
        self.start().await?;

        // Wait for cluster connectivity
        self.wait_for_cluster_connectivity().await?;

        info!("✅ Successfully joined cluster");
        Ok(())
    }

    /// Wait for cluster connectivity
    async fn wait_for_cluster_connectivity(&self) -> anyhow::Result<()> {
        info!("⏳ Waiting for cluster connectivity...");

        let timeout = std::time::Duration::from_secs(30);
        let start = std::time::Instant::now();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

        loop {
            interval.tick().await;

            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("Timeout waiting for cluster connectivity"));
            }

            // Check chitchat connectivity
            let chitchat_connected = {
                let chitchat = self.chitchat.read().await;
                chitchat.is_connected().await
            };

            if chitchat_connected {
                info!("✅ Cluster connectivity established");
                return Ok(());
            }
        }
    }

    /// Submit a request to the distributed state machine
    pub async fn submit_request(&self, request: RaftRequest) -> anyhow::Result<RaftResponse> {
        let raft = self.raft.read().await;
        raft.submit_request(request).await
    }

    /// Perform a consistent read
    pub async fn consistent_read(&self, request: RaftRequest) -> anyhow::Result<RaftResponse> {
        let raft = self.raft.read().await;
        raft.consistent_read(request).await
    }

    /// Check if this node is the leader
    pub async fn is_leader(&self) -> bool {
        let raft = self.raft.read().await;
        raft.is_leader().await
    }

    /// Get current leader node ID
    pub async fn get_leader(&self) -> Option<u64> {
        let raft = self.raft.read().await;
        raft.get_leader().await
    }

    /// Wait for leadership (either become leader or find a leader)
    pub async fn wait_for_leadership(&self) -> anyhow::Result<()> {
        let raft = self.raft.read().await;
        raft.wait_for_leadership().await
    }

    /// Discover services by type
    pub async fn discover_services(&self, service_type: &ServiceType) -> Vec<ServiceInfo> {
        self.service_registry.discover_services(service_type).await
    }

    /// Get cluster summary
    pub async fn get_cluster_summary(&self) -> ClusterSummary {
        self.service_registry.get_cluster_summary().await
    }

    /// Wait for a service to become available
    pub async fn wait_for_service(&self, 
                                  service_type: &ServiceType, 
                                  timeout: std::time::Duration) -> anyhow::Result<ServiceInfo> {
        self.service_registry.wait_for_service(service_type, timeout).await
    }

    /// Initialize cluster storage with default values
    pub async fn initialize_cluster_storage(&self) -> anyhow::Result<()> {
        if !self.is_leader().await {
            return Err(anyhow::anyhow!("Only leader can initialize cluster storage"));
        }

        info!("🌱 Initializing cluster storage with default values");

        // Set cluster metadata
        self.submit_request(RaftRequest::Set {
            key: "config:cluster_id".to_string(),
            value: "stract-cluster-001".to_string(),
        }).await?;

        self.submit_request(RaftRequest::Set {
            key: "config:shard_count".to_string(),
            value: "16".to_string(),
        }).await?;

        self.submit_request(RaftRequest::Set {
            key: "metadata:version".to_string(),
            value: "0.1.0".to_string(),
        }).await?;

        // Initialize stats
        self.submit_request(RaftRequest::Set {
            key: "stats:nodes_online".to_string(),
            value: "1".to_string(),
        }).await?;

        info!("✅ Cluster storage initialized successfully");
        Ok(())
    }

    /// Perform health check
    pub async fn health_check(&self) -> ClusterHealth {
        let mut health = ClusterHealth::default();

        // Check chitchat health
        health.chitchat_connected = {
            let chitchat = self.chitchat.read().await;
            chitchat.is_connected().await
        };

        // Check raft health
        health.raft_leader_elected = {
            let raft = self.raft.read().await;
            raft.get_leader().await.is_some()
        };

        health.is_leader = self.is_leader().await;

        // Get cluster summary
        let summary = self.get_cluster_summary().await;
        health.total_nodes = summary.total_nodes;
        health.healthy_nodes = summary.healthy_nodes;

        health.overall_healthy = health.chitchat_connected && 
                                health.raft_leader_elected && 
                                health.healthy_nodes > 0;

        health
    }

    /// Check if the coordinator is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Get service registry for direct access
    pub fn service_registry(&self) -> Arc<ServiceRegistry> {
        self.service_registry.clone()
    }

    /// Shutdown the cluster coordinator
    pub async fn shutdown(&self) {
        info!("🛑 Shutting down cluster coordinator");

        // Mark as not running
        *self.is_running.write().await = false;

        // Shutdown raft
        {
            let mut raft = self.raft.write().await;
            raft.shutdown().await;
        }

        // Shutdown chitchat
        {
            let mut chitchat = self.chitchat.write().await;
            chitchat.shutdown().await;
        }

        info!("✅ Cluster coordinator shutdown complete");
    }
}

/// Health status of the cluster
#[derive(Debug, Default)]
pub struct ClusterHealth {
    pub overall_healthy: bool,
    pub chitchat_connected: bool,
    pub raft_leader_elected: bool,
    pub is_leader: bool,
    pub total_nodes: usize,
    pub healthy_nodes: usize,
}

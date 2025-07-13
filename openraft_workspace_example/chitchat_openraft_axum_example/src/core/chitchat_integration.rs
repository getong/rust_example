//! Chitchat Integration Module
//! 
//! Simplified chitchat integration that follows the working patterns from cluster.rs

use chitchat::{spawn_chitchat, ChitchatHandle, transport::UdpTransport, FailureDetectorConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{info, warn, error, debug};

/// Service types that can be registered in the cluster
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ServiceType {
    /// DHT node for distributed hash table operations
    Dht { shard_id: u32 },
    /// Search server for handling search queries
    Search,
    /// API server for HTTP endpoints
    Api,
    /// Webgraph server for link analysis
    Webgraph { shard_id: u32 },
    /// AMPC Coordinator for distributed computing
    AmpcCoordinator { algorithm: String },
    /// AMPC Worker for distributed computing
    AmpcWorker { algorithm: String },
    /// Generic cluster node
    Node,
}

impl std::fmt::Display for ServiceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceType::Dht { shard_id } => write!(f, "dht-{}", shard_id),
            ServiceType::Search => write!(f, "search"),
            ServiceType::Api => write!(f, "api"),
            ServiceType::Webgraph { shard_id } => write!(f, "webgraph-{}", shard_id),
            ServiceType::AmpcCoordinator { algorithm } => write!(f, "coordinator-{}", algorithm),
            ServiceType::AmpcWorker { algorithm } => write!(f, "worker-{}", algorithm),
            ServiceType::Node => write!(f, "node"),
        }
    }
}

/// Simple chitchat manager following the working cluster.rs pattern
pub struct ChitchatManager {
    pub chitchat_handle: Option<ChitchatHandle>,
    pub node_id: String,
    pub listen_addr: SocketAddr,
    pub seed_nodes: Vec<SocketAddr>,
    pub service_type: ServiceType,
    pub service_addr: Option<SocketAddr>,
}

impl ChitchatManager {
    /// Create a new chitchat manager
    pub fn new(
        node_id: String,
        listen_addr: SocketAddr,
        seed_nodes: Vec<SocketAddr>,
        service_type: ServiceType,
    ) -> Self {
        Self {
            chitchat_handle: None,
            node_id,
            listen_addr,
            seed_nodes,
            service_type,
            service_addr: None,
        }
    }

    /// Set the service address for this node
    pub fn set_service_addr(&mut self, addr: SocketAddr) {
        self.service_addr = Some(addr);
    }

    /// Start the chitchat service following the working pattern
    pub async fn start(&mut self) -> anyhow::Result<()> {
        info!("🚀 Starting chitchat membership service on {}", self.listen_addr);

        let chitchat_config = chitchat::ChitchatConfig {
            cluster_id: "stract-cluster".to_string(),
            chitchat_id: chitchat::ChitchatId {
                node_id: self.node_id.clone(),
                generation_id: 0,
                gossip_advertise_addr: self.listen_addr,
            },
            gossip_interval: Duration::from_secs(1),
            listen_addr: self.listen_addr,
            seed_nodes: self.seed_nodes.iter().map(|addr| addr.to_string()).collect(),
            failure_detector_config: FailureDetectorConfig {
                dead_node_grace_period: Duration::from_secs(10),
                ..FailureDetectorConfig::default()
            },
            marked_for_deletion_grace_period: Duration::from_secs(60),
            catchup_callback: None,
            extra_liveness_predicate: None,
        };

        let chitchat_handle = spawn_chitchat(chitchat_config, Vec::new(), &UdpTransport).await?;

        // Register our service type
        self.register_service(&chitchat_handle).await?;

        self.chitchat_handle = Some(chitchat_handle);

        info!("✅ Chitchat membership service started successfully");
        Ok(())
    }

    /// Register service information
    async fn register_service(&self, handle: &ChitchatHandle) -> anyhow::Result<()> {
        let chitchat = handle.chitchat();
        let mut chitchat_guard = chitchat.lock().await;
        let state = chitchat_guard.self_node_state();

        // Set service type
        let service_type_json = serde_json::to_string(&self.service_type)?;
        state.set("service_type", &service_type_json);

        // Set service address if available
        if let Some(addr) = self.service_addr {
            state.set("service_addr", &addr.to_string());
        }

        // Set status
        state.set("status", "ready");

        debug!("📝 Registered service: type={}, addr={:?}", 
               self.service_type, self.service_addr);
        Ok(())
    }

    /// Check if connected to cluster
    pub async fn is_connected(&self) -> bool {
        if let Some(handle) = &self.chitchat_handle {
            let chitchat = handle.chitchat();
            let chitchat_guard = chitchat.lock().await;
            chitchat_guard.live_nodes().count() > 1 // More than just ourselves
        } else {
            false
        }
    }

    /// Get members count
    pub async fn get_member_count(&self) -> usize {
        if let Some(handle) = &self.chitchat_handle {
            let chitchat = handle.chitchat();
            let chitchat_guard = chitchat.lock().await;
            chitchat_guard.live_nodes().count()
        } else {
            0
        }
    }

    /// Get chitchat handle for direct access
    pub fn handle(&self) -> Option<&ChitchatHandle> {
        self.chitchat_handle.as_ref()
    }

    /// Shutdown chitchat
    pub async fn shutdown(&mut self) {
        if let Some(_handle) = &self.chitchat_handle {
            info!("🛑 Shutting down chitchat membership service");
            // Chitchat will automatically clean up
        }
        self.chitchat_handle = None;
    }
}

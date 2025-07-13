//! Service Registry Module
//! 
//! Manages service registration and discovery using chitchat for membership
//! and provides a unified interface for finding services in the cluster.

use crate::core::chitchat_integration::{ChitchatManager, ServiceType, MemberInfo};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, debug};

/// Service registry that tracks all services in the cluster
pub struct ServiceRegistry {
    chitchat: Arc<RwLock<ChitchatManager>>,
    service_cache: RwLock<HashMap<ServiceType, Vec<ServiceInfo>>>,
    cache_ttl: std::time::Duration,
    last_update: RwLock<std::time::Instant>,
}

/// Information about a discovered service
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub node_id: String,
    pub service_type: ServiceType,
    pub service_addr: SocketAddr,
    pub status: ServiceStatus,
    pub last_seen: std::time::Instant,
    pub metadata: HashMap<String, String>,
}

/// Status of a service
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceStatus {
    Ready,
    Starting,
    Stopping,
    Unhealthy,
    Unknown,
}

impl ServiceRegistry {
    /// Create a new service registry
    pub fn new(chitchat: Arc<RwLock<ChitchatManager>>) -> Self {
        Self {
            chitchat,
            service_cache: RwLock::new(HashMap::new()),
            cache_ttl: std::time::Duration::from_secs(10),
            last_update: RwLock::new(std::time::Instant::now()),
        }
    }

    /// Start the service registry background tasks
    pub async fn start(&self) {
        info!("🚀 Starting service registry");
        
        // Start cache refresh task
        let registry_clone = Arc::new(self.clone());
        tokio::spawn(async move {
            registry_clone.cache_refresh_loop().await;
        });
        
        info!("✅ Service registry started");
    }

    /// Background task to refresh the service cache
    async fn cache_refresh_loop(self: Arc<Self>) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.refresh_cache().await {
                warn!("Failed to refresh service cache: {}", e);
            }
        }
    }

    /// Refresh the service cache from chitchat
    async fn refresh_cache(&self) -> anyhow::Result<()> {
        let chitchat = self.chitchat.read().await;
        let members = chitchat.get_members().await;
        drop(chitchat);

        let mut new_cache = HashMap::new();
        
        for (_, member) in members {
            if let Some(service_addr) = member.service_addr {
                let service_info = ServiceInfo {
                    node_id: member.node_id.node_id.clone(),
                    service_type: member.service_type.clone(),
                    service_addr,
                    status: Self::determine_status(&member),
                    last_seen: member.last_seen,
                    metadata: HashMap::new(),
                };

                new_cache
                    .entry(member.service_type)
                    .or_insert_with(Vec::new)
                    .push(service_info);
            }
        }

        // Update cache
        *self.service_cache.write().await = new_cache;
        *self.last_update.write().await = std::time::Instant::now();
        
        debug!("🔄 Refreshed service cache with {} service types", 
               self.service_cache.read().await.len());
        
        Ok(())
    }

    /// Determine service status from member info
    fn determine_status(member: &MemberInfo) -> ServiceStatus {
        // Simple heuristic based on last seen time
        let elapsed = member.last_seen.elapsed();
        
        if elapsed < std::time::Duration::from_secs(10) {
            ServiceStatus::Ready
        } else if elapsed < std::time::Duration::from_secs(30) {
            ServiceStatus::Unhealthy
        } else {
            ServiceStatus::Unknown
        }
    }

    /// Discover services by type
    pub async fn discover_services(&self, service_type: &ServiceType) -> Vec<ServiceInfo> {
        // Check if cache needs refresh
        if self.last_update.read().await.elapsed() > self.cache_ttl {
            let _ = self.refresh_cache().await;
        }

        self.service_cache.read().await
            .get(service_type)
            .cloned()
            .unwrap_or_default()
    }

    /// Discover all DHT nodes grouped by shard
    pub async fn discover_dht_shards(&self) -> HashMap<u32, Vec<ServiceInfo>> {
        let mut shard_map = HashMap::new();
        
        let cache = self.service_cache.read().await;
        for (service_type, services) in cache.iter() {
            if let ServiceType::Dht { shard_id } = service_type {
                shard_map.entry(*shard_id).or_insert_with(Vec::new).extend(services.clone());
            }
        }
        
        shard_map
    }

    /// Discover services by status
    pub async fn discover_healthy_services(&self, service_type: &ServiceType) -> Vec<ServiceInfo> {
        self.discover_services(service_type).await
            .into_iter()
            .filter(|service| service.status == ServiceStatus::Ready)
            .collect()
    }

    /// Get a random healthy service of the given type
    pub async fn get_random_service(&self, service_type: &ServiceType) -> Option<ServiceInfo> {
        let healthy_services = self.discover_healthy_services(service_type).await;
        
        if healthy_services.is_empty() {
            None
        } else {
            use rand::seq::SliceRandom;
            let mut rng = rand::thread_rng();
            healthy_services.choose(&mut rng).cloned()
        }
    }

    /// Get service by node ID
    pub async fn get_service_by_node(&self, node_id: &str) -> Option<ServiceInfo> {
        let cache = self.service_cache.read().await;
        
        for services in cache.values() {
            for service in services {
                if service.node_id == node_id {
                    return Some(service.clone());
                }
            }
        }
        
        None
    }

    /// Get cluster summary
    pub async fn get_cluster_summary(&self) -> ClusterSummary {
        let cache = self.service_cache.read().await;
        let mut summary = ClusterSummary::default();
        
        for (service_type, services) in cache.iter() {
            match service_type {
                ServiceType::Dht { .. } => summary.dht_nodes += services.len(),
                ServiceType::Search => summary.search_nodes += services.len(),
                ServiceType::Api => summary.api_nodes += services.len(),
                ServiceType::Webgraph { .. } => summary.webgraph_nodes += services.len(),
                ServiceType::AmpcCoordinator { .. } => summary.coordinator_nodes += services.len(),
                ServiceType::AmpcWorker { .. } => summary.worker_nodes += services.len(),
                ServiceType::Node => summary.generic_nodes += services.len(),
            }
            
            summary.total_nodes += services.len();
            
            // Count healthy services
            summary.healthy_nodes += services.iter()
                .filter(|s| s.status == ServiceStatus::Ready)
                .count();
        }
        
        summary
    }

    /// Wait for a service type to become available
    pub async fn wait_for_service(&self, 
                                  service_type: &ServiceType, 
                                  timeout: std::time::Duration) -> anyhow::Result<ServiceInfo> {
        let start = std::time::Instant::now();
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
        
        loop {
            interval.tick().await;
            
            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("Timeout waiting for service: {}", service_type));
            }
            
            if let Some(service) = self.get_random_service(service_type).await {
                return Ok(service);
            }
        }
    }
}

impl Clone for ServiceRegistry {
    fn clone(&self) -> Self {
        Self {
            chitchat: self.chitchat.clone(),
            service_cache: RwLock::new(HashMap::new()),
            cache_ttl: self.cache_ttl,
            last_update: RwLock::new(std::time::Instant::now()),
        }
    }
}

/// Summary of cluster services
#[derive(Debug, Default)]
pub struct ClusterSummary {
    pub total_nodes: usize,
    pub healthy_nodes: usize,
    pub dht_nodes: usize,
    pub search_nodes: usize,
    pub api_nodes: usize,
    pub webgraph_nodes: usize,
    pub coordinator_nodes: usize,
    pub worker_nodes: usize,
    pub generic_nodes: usize,
}

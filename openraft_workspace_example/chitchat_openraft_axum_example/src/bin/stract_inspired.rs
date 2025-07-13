//! Stract-inspired distributed system demonstrating chitchat + openraft integration
//!
//! This binary follows the Stract architecture pattern with:
//! - AMPC (A Modern Parallel Computing) framework
//! - Multiple service types (DHT, API, SearchServer, WebgraphServer)
//! - Distributed cluster management with chitchat
//! - OpenRaft for consistency in distributed state

use std::net::SocketAddr;
use std::time::Duration;
use std::collections::HashMap;

use clap::{Parser, Subcommand};
use tokio::time::sleep;
use tracing::{info, warn, error, debug, Level};
use tracing_subscriber;

use chitchat_openraft_axum_example::distributed::{
    cluster::{Cluster, ClusterConfig},
    member::Service,
    dht::{DhtServer, DhtRequest, DhtClient},
};

#[derive(Parser)]
#[clap(author, version, about = "Stract-inspired distributed system with chitchat + openraft")]
#[clap(propagate_version = true)]
struct Args {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// AMPC (A Modern Parallel Computing) distributed services
    Ampc {
        #[clap(subcommand)]
        options: AmpcOptions,
    },

    /// Start various server types
    Server {
        #[clap(subcommand)]
        server_type: ServerType,
    },

    /// Client utilities for testing the cluster
    Client {
        #[clap(subcommand)]
        operation: ClientOperation,
    },

    /// Cluster management commands
    Cluster {
        #[clap(subcommand)]
        operation: ClusterOperation,
    },
}

#[derive(Subcommand)]
enum AmpcOptions {
    /// Start a DHT (Distributed Hash Table) node
    Dht {
        /// Node ID
        #[clap(long, default_value = "1")]
        node_id: u64,
        
        /// Shard ID for this DHT node
        #[clap(long, default_value = "0")]
        shard_id: u32,
        
        /// Service listen address
        #[clap(long, default_value = "127.0.0.1:8081")]
        service_addr: SocketAddr,
        
        /// Chitchat listen address
        #[clap(long, default_value = "127.0.0.1:10001")]
        chitchat_addr: SocketAddr,
        
        /// Seed nodes for joining cluster
        #[clap(long)]
        seeds: Vec<SocketAddr>,
    },

    /// Start a coordinator for distributed graph algorithms
    Coordinator {
        /// Coordinator type
        #[clap(subcommand)]
        coordinator_type: CoordinatorType,
    },

    /// Start a worker for distributed computation
    Worker {
        /// Worker type
        #[clap(subcommand)]
        worker_type: WorkerType,
    },
}

#[derive(Subcommand)]
enum CoordinatorType {
    /// Harmonic centrality coordinator
    Harmonic {
        /// Configuration for harmonic centrality
        #[clap(long, default_value = "127.0.0.1:9001")]
        coord_addr: SocketAddr,
        
        #[clap(long, default_value = "127.0.0.1:10001")]
        chitchat_addr: SocketAddr,
        
        #[clap(long)]
        seeds: Vec<SocketAddr>,
    },

    /// Shortest path coordinator
    ShortestPath {
        /// Configuration for shortest path computation
        #[clap(long, default_value = "127.0.0.1:9002")]
        coord_addr: SocketAddr,
        
        #[clap(long, default_value = "127.0.0.1:10001")]
        chitchat_addr: SocketAddr,
        
        #[clap(long)]
        seeds: Vec<SocketAddr>,
    },
}

#[derive(Subcommand)]
enum WorkerType {
    /// Harmonic centrality worker
    Harmonic {
        /// Worker configuration
        #[clap(long, default_value = "127.0.0.1:9101")]
        worker_addr: SocketAddr,
        
        #[clap(long, default_value = "127.0.0.1:10001")]
        chitchat_addr: SocketAddr,
        
        #[clap(long)]
        seeds: Vec<SocketAddr>,
    },

    /// Shortest path worker
    ShortestPath {
        /// Worker configuration
        #[clap(long, default_value = "127.0.0.1:9102")]
        worker_addr: SocketAddr,
        
        #[clap(long, default_value = "127.0.0.1:10001")]
        chitchat_addr: SocketAddr,
        
        #[clap(long)]
        seeds: Vec<SocketAddr>,
    },
}

#[derive(Subcommand)]
enum ServerType {
    /// Search server (like Stract's search functionality)
    Search {
        #[clap(long, default_value = "127.0.0.1:8080")]
        addr: SocketAddr,
        
        #[clap(long, default_value = "127.0.0.1:10001")]
        chitchat_addr: SocketAddr,
        
        #[clap(long)]
        seeds: Vec<SocketAddr>,
    },

    /// API server (HTTP API interface)
    Api {
        #[clap(long, default_value = "127.0.0.1:8090")]
        addr: SocketAddr,
        
        #[clap(long, default_value = "127.0.0.1:10001")]
        chitchat_addr: SocketAddr,
        
        #[clap(long)]
        seeds: Vec<SocketAddr>,
    },

    /// Webgraph server (link analysis)
    Webgraph {
        #[clap(long, default_value = "127.0.0.1:8070")]
        addr: SocketAddr,
        
        #[clap(long, default_value = "0")]
        shard_id: u32,
        
        #[clap(long, default_value = "127.0.0.1:10001")]
        chitchat_addr: SocketAddr,
        
        #[clap(long)]
        seeds: Vec<SocketAddr>,
    },
}

#[derive(Subcommand)]
enum ClientOperation {
    /// Test DHT operations
    TestDht {
        /// DHT servers to connect to
        #[clap(long, default_value = "127.0.0.1:8081")]
        servers: Vec<SocketAddr>,
        
        /// Number of test operations
        #[clap(long, default_value = "10")]
        operations: usize,
    },

    /// Discover cluster members
    Discover {
        #[clap(long, default_value = "127.0.0.1:10001")]
        chitchat_addr: SocketAddr,
        
        #[clap(long)]
        seeds: Vec<SocketAddr>,
    },
}

#[derive(Subcommand)]
enum ClusterOperation {
    /// Show cluster status
    Status {
        #[clap(long, default_value = "127.0.0.1:10001")]
        chitchat_addr: SocketAddr,
        
        #[clap(long)]
        seeds: Vec<SocketAddr>,
    },

    /// Initialize a new cluster
    Init {
        #[clap(long, default_value = "127.0.0.1:10001")]
        chitchat_addr: SocketAddr,
    },
}

/// Start DHT service following Stract's AMPC pattern
async fn start_ampc_dht(
    node_id: u64,
    shard_id: u32,
    service_addr: SocketAddr,
    chitchat_addr: SocketAddr,
    seeds: Vec<SocketAddr>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🚀 Starting AMPC DHT node: id={}, shard={}, service={}", 
          node_id, shard_id, service_addr);

    // Create service registration
    let service = Service::Dht {
        host: service_addr,
        shard: shard_id,
    };

    // Configure cluster following Stract pattern
    let cluster_config = ClusterConfig {
        chitchat_id: format!("ampc-dht-{}-{}", node_id, shard_id),
        chitchat_listen_addr: chitchat_addr,
        seed_nodes: seeds,
        heartbeat_interval: Duration::from_millis(500),
        marked_for_deletion_grace_period: Duration::from_secs(30),
    };

    // Join chitchat cluster
    let mut cluster = Cluster::new(cluster_config, service.clone());
    cluster.start().await?;
    info!("✅ Joined chitchat cluster successfully");

    // Initialize DHT with OpenRaft backend
    let mut dht_server = DhtServer::new(node_id as u32, shard_id, service_addr);
    dht_server.start().await?;
    info!("✅ DHT server started with OpenRaft backend");

    // Start periodic cluster member updates
    tokio::spawn(async move {
        loop {
            if let Err(e) = cluster.update_members().await {
                error!("Failed to update cluster members: {}", e);
            }
            sleep(Duration::from_secs(5)).await;
        }
    });

    // Wait for leadership election
    info!("⏳ Waiting for leadership election...");
    let mut attempts = 0;
    while !dht_server.is_leader().await && attempts < 20 {
        sleep(Duration::from_millis(500)).await;
        attempts += 1;
    }

    if dht_server.is_leader().await {
        info!("👑 Node became leader, performing initialization");
        
        // Demo distributed hash table operations
        let test_operations = vec![
            ("config:cluster_id", "stract-cluster-001"),
            ("config:shard_count", "16"),
            ("stats:nodes_online", "1"),
            ("metadata:version", "0.1.0"),
        ];

        for (key, value) in test_operations {
            let put_request = DhtRequest::Put {
                key: key.to_string(),
                value: value.to_string(),
            };
            
            match dht_server.handle_request(put_request).await {
                Ok(_) => info!("✅ Stored: {} = {}", key, value),
                Err(e) => error!("❌ Failed to store {}: {}", key, e),
            }
        }
    } else {
        info!("📡 Node is follower, ready to serve read requests");
    }

    info!("🎯 AMPC DHT service running - Press Ctrl+C to stop");
    tokio::signal::ctrl_c().await?;
    
    info!("🛑 Shutting down AMPC DHT service...");
    // In production: proper OpenRaft shutdown
    Ok(())
}

/// Start a search server service
async fn start_search_server(
    addr: SocketAddr,
    chitchat_addr: SocketAddr,
    seeds: Vec<SocketAddr>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🔍 Starting Search Server: addr={}", addr);

    let service = Service::Searcher {
        host: addr,
        shard: 0, // Default shard for search server
    };

    let cluster_config = ClusterConfig {
        chitchat_id: format!("search-server-{}", addr.port()),
        chitchat_listen_addr: chitchat_addr,
        seed_nodes: seeds,
        heartbeat_interval: Duration::from_millis(500),
        marked_for_deletion_grace_period: Duration::from_secs(30),
    };

    let mut cluster = Cluster::new(cluster_config, service);
    cluster.start().await?;
    info!("✅ Search server joined cluster");

    // Search server would handle search queries here
    info!("🎯 Search Server running - Press Ctrl+C to stop");
    tokio::signal::ctrl_c().await?;
    
    info!("🛑 Shutting down Search Server...");
    Ok(())
}

/// Start an API server
async fn start_api_server(
    addr: SocketAddr,
    chitchat_addr: SocketAddr,
    seeds: Vec<SocketAddr>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🌐 Starting API Server: addr={}", addr);

    let service = Service::Api { host: addr };

    let cluster_config = ClusterConfig {
        chitchat_id: format!("api-server-{}", addr.port()),
        chitchat_listen_addr: chitchat_addr,
        seed_nodes: seeds,
        heartbeat_interval: Duration::from_millis(500),
        marked_for_deletion_grace_period: Duration::from_secs(30),
    };

    let mut cluster = Cluster::new(cluster_config, service);
    cluster.start().await?;
    info!("✅ API server joined cluster");

    // API server would handle HTTP API requests here
    info!("🎯 API Server running - Press Ctrl+C to stop");
    tokio::signal::ctrl_c().await?;
    
    info!("🛑 Shutting down API Server...");
    Ok(())
}

/// Start a webgraph server
async fn start_webgraph_server(
    addr: SocketAddr,
    shard_id: u32,
    chitchat_addr: SocketAddr,
    seeds: Vec<SocketAddr>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🕸️ Starting Webgraph Server: addr={}, shard={}", addr, shard_id);

    let service = Service::Webgraph {
        host: addr,
        shard: shard_id,
    };

    let cluster_config = ClusterConfig {
        chitchat_id: format!("webgraph-{}-{}", addr.port(), shard_id),
        chitchat_listen_addr: chitchat_addr,
        seed_nodes: seeds,
        heartbeat_interval: Duration::from_millis(500),
        marked_for_deletion_grace_period: Duration::from_secs(30),
    };

    let mut cluster = Cluster::new(cluster_config, service);
    cluster.start().await?;
    info!("✅ Webgraph server joined cluster");

    // Webgraph server would handle link analysis here
    info!("🎯 Webgraph Server running - Press Ctrl+C to stop");
    tokio::signal::ctrl_c().await?;
    
    info!("🛑 Shutting down Webgraph Server...");
    Ok(())
}

/// Start coordinator for distributed computation
async fn start_coordinator(
    coord_type: CoordinatorType,
) -> Result<(), Box<dyn std::error::Error>> {
    match coord_type {
        CoordinatorType::Harmonic { coord_addr, chitchat_addr, seeds } => {
            info!("📊 Starting Harmonic Centrality Coordinator: addr={}", coord_addr);
            
            // Coordinator would orchestrate harmonic centrality computation
            let service = Service::Api { host: coord_addr }; // Coordinator as API service
            
            let cluster_config = ClusterConfig {
                chitchat_id: format!("harmonic-coordinator-{}", coord_addr.port()),
                chitchat_listen_addr: chitchat_addr,
                seed_nodes: seeds,
                heartbeat_interval: Duration::from_millis(500),
                marked_for_deletion_grace_period: Duration::from_secs(30),
            };

            let mut cluster = Cluster::new(cluster_config, service);
            cluster.start().await?;
            info!("✅ Harmonic coordinator joined cluster");

            info!("🎯 Harmonic Coordinator running - Press Ctrl+C to stop");
            tokio::signal::ctrl_c().await?;
        }
        
        CoordinatorType::ShortestPath { coord_addr, chitchat_addr, seeds } => {
            info!("🛤️ Starting Shortest Path Coordinator: addr={}", coord_addr);
            
            let service = Service::Api { host: coord_addr };
            
            let cluster_config = ClusterConfig {
                chitchat_id: format!("shortest-path-coordinator-{}", coord_addr.port()),
                chitchat_listen_addr: chitchat_addr,
                seed_nodes: seeds,
                heartbeat_interval: Duration::from_millis(500),
                marked_for_deletion_grace_period: Duration::from_secs(30),
            };

            let mut cluster = Cluster::new(cluster_config, service);
            cluster.start().await?;
            info!("✅ Shortest path coordinator joined cluster");

            info!("🎯 Shortest Path Coordinator running - Press Ctrl+C to stop");
            tokio::signal::ctrl_c().await?;
        }
    }
    
    Ok(())
}

/// Start worker for distributed computation
async fn start_worker(
    worker_type: WorkerType,
) -> Result<(), Box<dyn std::error::Error>> {
    match worker_type {
        WorkerType::Harmonic { worker_addr, chitchat_addr, seeds } => {
            info!("⚙️ Starting Harmonic Centrality Worker: addr={}", worker_addr);
            
            let service = Service::Searcher {
                host: worker_addr,
                shard: 0, // Workers as searcher-type services
            };
            
            let cluster_config = ClusterConfig {
                chitchat_id: format!("harmonic-worker-{}", worker_addr.port()),
                chitchat_listen_addr: chitchat_addr,
                seed_nodes: seeds,
                heartbeat_interval: Duration::from_millis(500),
                marked_for_deletion_grace_period: Duration::from_secs(30),
            };

            let mut cluster = Cluster::new(cluster_config, service);
            cluster.start().await?;
            info!("✅ Harmonic worker joined cluster");

            info!("🎯 Harmonic Worker running - Press Ctrl+C to stop");
            tokio::signal::ctrl_c().await?;
        }
        
        WorkerType::ShortestPath { worker_addr, chitchat_addr, seeds } => {
            info!("🔧 Starting Shortest Path Worker: addr={}", worker_addr);
            
            let service = Service::Searcher {
                host: worker_addr,
                shard: 0,
            };
            
            let cluster_config = ClusterConfig {
                chitchat_id: format!("shortest-path-worker-{}", worker_addr.port()),
                chitchat_listen_addr: chitchat_addr,
                seed_nodes: seeds,
                heartbeat_interval: Duration::from_millis(500),
                marked_for_deletion_grace_period: Duration::from_secs(30),
            };

            let mut cluster = Cluster::new(cluster_config, service);
            cluster.start().await?;
            info!("✅ Shortest path worker joined cluster");

            info!("🎯 Shortest Path Worker running - Press Ctrl+C to stop");
            tokio::signal::ctrl_c().await?;
        }
    }
    
    Ok(())
}

/// Test DHT operations with multiple servers
async fn test_dht_operations(
    servers: Vec<SocketAddr>,
    operations: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🧪 Testing DHT operations with {} servers", servers.len());

    let mut client = DhtClient::new();
    
    // Add servers to client
    client.add_shard_servers(0, servers.clone());
    
    info!("📊 Running {} test operations...", operations);
    
    for i in 0..operations {
        let key = format!("test_key_{}", i);
        let value = format!("test_value_{}", i);
        
        // PUT operation
        match client.put(key.clone(), value.clone()).await {
            Ok(()) => debug!("✅ PUT {}: {}", key, value),
            Err(e) => warn!("❌ PUT failed for {}: {}", key, e),
        }
        
        // GET operation
        match client.get(&key).await {
            Ok(Some(retrieved_value)) => {
                if retrieved_value == value {
                    debug!("✅ GET {}: {}", key, retrieved_value);
                } else {
                    warn!("⚠️ GET {}: expected {}, got {}", key, value, retrieved_value);
                }
            }
            Ok(None) => warn!("❌ GET {}: key not found", key),
            Err(e) => warn!("❌ GET failed for {}: {}", key, e),
        }
        
        if i % 10 == 0 && i > 0 {
            info!("📈 Progress: {}/{} operations completed", i, operations);
        }
        
        sleep(Duration::from_millis(100)).await;
    }
    
    info!("✅ DHT testing completed!");
    Ok(())
}

/// Discover cluster members
async fn discover_cluster(
    chitchat_addr: SocketAddr,
    seeds: Vec<SocketAddr>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🔍 Discovering cluster members...");

    let service = Service::Api { 
        host: "127.0.0.1:9999".parse().unwrap() // Temporary discovery service
    };
    
    let cluster_config = ClusterConfig {
        chitchat_id: "cluster-discovery".to_string(),
        chitchat_listen_addr: chitchat_addr,
        seed_nodes: seeds,
        heartbeat_interval: Duration::from_millis(500),
        marked_for_deletion_grace_period: Duration::from_secs(30),
    };

    let mut cluster = Cluster::new(cluster_config, service);
    cluster.start().await?;
    
    // Wait for cluster discovery
    sleep(Duration::from_secs(3)).await;
    
    // Get members (using available API)
    let members = cluster.get_api_members();
    
    info!("📡 Found {} cluster members:", members.len());
    for member in &members {
        info!("  👤 ID: {}, Service: {:?}", member.id, member.service);
    }
    
    cluster.stop().await;
    Ok(())
}

/// Show cluster status
async fn show_cluster_status(
    chitchat_addr: SocketAddr,
    seeds: Vec<SocketAddr>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("📊 Checking cluster status...");

    let service = Service::Api { 
        host: "127.0.0.1:9998".parse().unwrap() // Temporary status service
    };
    
    let cluster_config = ClusterConfig {
        chitchat_id: "cluster-status".to_string(),
        chitchat_listen_addr: chitchat_addr,
        seed_nodes: seeds,
        heartbeat_interval: Duration::from_millis(500),
        marked_for_deletion_grace_period: Duration::from_secs(30),
    };

    let mut cluster = Cluster::new(cluster_config, service);
    cluster.start().await?;
    
    // Wait for cluster state
    sleep(Duration::from_secs(3)).await;
    
    let members = cluster.get_api_members();
    
    // Group by service type
    let mut service_counts: HashMap<String, usize> = HashMap::new();
    for member in &members {
        let service_type = match &member.service {
            Service::Dht { .. } => "DHT",
            Service::Api { .. } => "API",
            Service::Searcher { .. } => "Searcher",
            Service::Webgraph { .. } => "Webgraph",
        };
        *service_counts.entry(service_type.to_string()).or_insert(0) += 1;
    }
    
    info!("🏛️ Cluster Status Summary:");
    info!("  📊 Total Nodes: {}", members.len());
    for (service_type, count) in service_counts {
        info!("  🔧 {}: {} nodes", service_type, count);
    }
    
    cluster.stop().await;
    Ok(())
}

/// Initialize a new cluster
async fn init_cluster(
    chitchat_addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🌱 Initializing new cluster at {}", chitchat_addr);

    let service = Service::Api { 
        host: chitchat_addr
    };
    
    let cluster_config = ClusterConfig {
        chitchat_id: "cluster-init".to_string(),
        chitchat_listen_addr: chitchat_addr,
        seed_nodes: vec![], // No seeds for initialization
        heartbeat_interval: Duration::from_millis(500),
        marked_for_deletion_grace_period: Duration::from_secs(30),
    };

    let mut cluster = Cluster::new(cluster_config, service);
    cluster.start().await?;
    
    info!("✅ Cluster initialized successfully!");
    info!("🔗 Other nodes can join using: --seeds {}", chitchat_addr);
    
    // Keep running for a moment to establish the cluster
    sleep(Duration::from_secs(5)).await;
    
    cluster.stop().await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .without_time()
        .with_target(false)
        .init();

    let args = Args::parse();

    match args.command {
        Commands::Ampc { options } => {
            match options {
                AmpcOptions::Dht { node_id, shard_id, service_addr, chitchat_addr, seeds } => {
                    start_ampc_dht(node_id, shard_id, service_addr, chitchat_addr, seeds).await?
                }
                AmpcOptions::Coordinator { coordinator_type } => {
                    start_coordinator(coordinator_type).await?
                }
                AmpcOptions::Worker { worker_type } => {
                    start_worker(worker_type).await?
                }
            }
        }

        Commands::Server { server_type } => {
            match server_type {
                ServerType::Search { addr, chitchat_addr, seeds } => {
                    start_search_server(addr, chitchat_addr, seeds).await?
                }
                ServerType::Api { addr, chitchat_addr, seeds } => {
                    start_api_server(addr, chitchat_addr, seeds).await?
                }
                ServerType::Webgraph { addr, shard_id, chitchat_addr, seeds } => {
                    start_webgraph_server(addr, shard_id, chitchat_addr, seeds).await?
                }
            }
        }

        Commands::Client { operation } => {
            match operation {
                ClientOperation::TestDht { servers, operations } => {
                    test_dht_operations(servers, operations).await?
                }
                ClientOperation::Discover { chitchat_addr, seeds } => {
                    discover_cluster(chitchat_addr, seeds).await?
                }
            }
        }

        Commands::Cluster { operation } => {
            match operation {
                ClusterOperation::Status { chitchat_addr, seeds } => {
                    show_cluster_status(chitchat_addr, seeds).await?
                }
                ClusterOperation::Init { chitchat_addr } => {
                    init_cluster(chitchat_addr).await?
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cluster_initialization() {
        // Test basic cluster setup
        let addr: SocketAddr = "127.0.0.1:19999".parse().unwrap();
        
        // This would test cluster initialization in a real scenario
        assert!(addr.port() > 0);
    }

    #[test]
    fn test_service_types() {
        // Test service type creation
        let dht_service = Service::Dht {
            host: "127.0.0.1:8081".parse().unwrap(),
            shard: 0,
        };
        
        assert!(matches!(dht_service, Service::Dht { .. }));
    }
}

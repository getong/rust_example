//! Production OpenRaft example following Stract's architecture
//!
//! This example demonstrates the complete integration of chitchat and OpenRaft
//! using our current working implementation as a foundation.

use std::net::SocketAddr;
use std::time::Duration;

use clap::Parser;
use tokio::time::sleep;
use tracing::{info, warn, error, Level};
use tracing_subscriber;

use chitchat_openraft_axum_example::distributed::{
    cluster::{Cluster, ClusterConfig},
    member::{Service, ShardId},
    dht::{DhtServer, DhtRequest, DhtClient},
};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Node ID
    #[clap(long, default_value = "1")]
    node_id: u64,

    /// Service type
    #[clap(long)]
    service_type: ServiceType,

    /// Chitchat listen port
    #[clap(long, default_value = "10001")]
    chitchat_port: u16,

    /// Service listen port
    #[clap(long, default_value = "8081")]
    service_port: u16,

    /// Shard ID (for sharded services)
    #[clap(long, default_value = "0")]
    shard_id: u64,

    /// Seed nodes for joining cluster
    #[clap(long)]
    seeds: Vec<String>,

    /// Run in client mode (send test requests)
    #[clap(long)]
    client_mode: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum ServiceType {
    Dht,
    Api,
    Searcher,
    Webgraph,
}

impl From<ServiceType> for Service {
    fn from(service_type: ServiceType) -> Self {
        match service_type {
            ServiceType::Dht => Service::Dht {
                host: "127.0.0.1:8081".parse().unwrap(),
                shard: ShardId::from(0u32),
            },
            ServiceType::Api => Service::Api {
                host: "127.0.0.1:8081".parse().unwrap(),
            },
            ServiceType::Searcher => Service::Searcher {
                host: "127.0.0.1:8081".parse().unwrap(),
                shard: ShardId::from(0u32),
            },
            ServiceType::Webgraph => Service::Webgraph {
                host: "127.0.0.1:8081".parse().unwrap(),
                shard: ShardId::from(0u32),
            },
        }
    }
}

async fn start_dht_service(
    node_id: u64,
    shard_id: ShardId,
    service_addr: SocketAddr,
    chitchat_addr: SocketAddr,
    seeds: Vec<SocketAddr>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting DHT service: node_id={}, shard_id={}, service_addr={}", 
          node_id, shard_id, service_addr);

    // Create the service registration
    let service = Service::Dht {
        host: service_addr,
        shard: shard_id,
    };

    // Create cluster configuration using our actual API
    let cluster_config = ClusterConfig {
        chitchat_id: format!("dht-{}-{}", node_id, shard_id),
        chitchat_listen_addr: chitchat_addr,
        seed_nodes: seeds,
        heartbeat_interval: Duration::from_millis(500),
        marked_for_deletion_grace_period: Duration::from_secs(30),
    };

    // Join the chitchat cluster with our actual API
    let mut cluster = Cluster::new(cluster_config, service.clone());
    cluster.start().await?;

    info!("DHT cluster started successfully");

    // Create and start the DHT server using our working implementation
    let mut dht_server = DhtServer::new(node_id as u32, shard_id, service_addr);
    dht_server.start().await?;

    info!("DHT service started successfully, waiting for cluster membership...");

    // Wait for cluster to form
    sleep(Duration::from_secs(2)).await;

    // Update cluster members periodically
    tokio::spawn(async move {
        loop {
            if let Err(e) = cluster.update_members().await {
                error!("Failed to update cluster members: {}", e);
            }
            sleep(Duration::from_secs(5)).await;
        }
    });

    // Handle some test operations
    info!("DHT server is ready, testing operations...");
    
    // Wait to become leader if needed
    let mut attempts = 0;
    while !dht_server.is_leader().await && attempts < 10 {
        sleep(Duration::from_millis(500)).await;
        attempts += 1;
    }

    if dht_server.is_leader().await {
        info!("Node is leader, performing test operations");
        
        // Test put operation using our actual DhtRequest types
        let put_request = DhtRequest::Put {
            key: "test_key".to_string(),
            value: "test_value".to_string(),
        };
        
        match dht_server.handle_request(put_request).await {
            Ok(response) => info!("PUT operation successful: {:?}", response),
            Err(e) => error!("PUT operation failed: {}", e),
        }

        // Test get operation
        let get_request = DhtRequest::Get {
            key: "test_key".to_string(),
        };
        
        match dht_server.handle_request(get_request).await {
            Ok(response) => info!("GET operation successful: {:?}", response),
            Err(e) => error!("GET operation failed: {}", e),
        }
    } else {
        info!("Node is not leader, will handle read operations only");
    }

    // Keep the service running
    info!("DHT service running, press Ctrl+C to stop");
    
    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    
    info!("Shutting down DHT service...");
    // Note: DhtServer doesn't have a stop method in our implementation
    // In a production system, you would properly shutdown OpenRaft
    
    Ok(())
}

async fn run_client_mode(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting in client mode...");
    
    let mut client = DhtClient::new();
    
    // Add some test servers (in a real scenario, these would be discovered via chitchat)
    let mut servers = Vec::new();
    for port in 8081..8084 {
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse()?;
        servers.push(addr);
    }
    client.add_shard_servers(0u32, servers);
    
    // Test operations
    info!("Testing PUT operation...");
    match client.put("client_test_key".to_string(), "client_test_value".to_string()).await {
        Ok(()) => info!("PUT operation successful"),
        Err(e) => warn!("PUT operation failed: {}", e),
    }
    
    sleep(Duration::from_secs(1)).await;
    
    info!("Testing GET operation...");
    match client.get("client_test_key").await {
        Ok(Some(value)) => info!("GET operation successful: {}", value),
        Ok(None) => info!("GET operation: key not found"),
        Err(e) => warn!("GET operation failed: {}", e),
    }
    
    info!("Testing DELETE operation...");
    match client.delete("client_test_key").await {
        Ok(existed) => info!("DELETE operation successful: existed={}", existed),
        Err(e) => warn!("DELETE operation failed: {}", e),
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let args = Args::parse();
    
    if args.client_mode {
        return run_client_mode(args).await;
    }

    // Parse addresses
    let chitchat_addr: SocketAddr = format!("127.0.0.1:{}", args.chitchat_port).parse()?;
    let service_addr: SocketAddr = format!("127.0.0.1:{}", args.service_port).parse()?;
    
    // Parse seed addresses
    let seeds: Result<Vec<SocketAddr>, _> = args.seeds
        .iter()
        .map(|s| s.parse())
        .collect();
    let seeds = seeds?;

    match args.service_type {
        ServiceType::Dht => {
            start_dht_service(
                args.node_id,
                args.shard_id as u32,
                service_addr,
                chitchat_addr,
                seeds,
            )
            .await
        }
        ServiceType::Api | ServiceType::Searcher | ServiceType::Webgraph => {
            warn!("Service type {:?} not implemented in this example", args.service_type);
            info!("Only DHT service is implemented. Use --service-type dht");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_type_conversion() {
        let dht_service: Service = ServiceType::Dht.into();
        assert!(matches!(dht_service, Service::Dht { .. }));
        
        let api_service: Service = ServiceType::Api.into();
        assert!(matches!(api_service, Service::Api { .. }));
    }
}

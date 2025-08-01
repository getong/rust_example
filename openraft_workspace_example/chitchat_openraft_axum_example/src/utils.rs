use std::net::SocketAddr;

use cool_id_generator::Size;

use crate::distributed::{Service, ShardId};

pub fn generate_server_id(public_addr: SocketAddr) -> String {
  let cool_id = cool_id_generator::get_id(Size::Medium);
  format!("server:{public_addr}-{cool_id}")
}

pub fn create_service(service_type: &str, host: SocketAddr, shard: Option<u64>) -> Service {
  match service_type {
    "searcher" => Service::Searcher {
      host,
      shard: ShardId::new(shard.unwrap_or(0)),
    },
    "api_gateway" => Service::ApiGateway { host },
    "data_processor" => Service::DataProcessor {
      host,
      shard: ShardId::new(shard.unwrap_or(0)),
    },
    "storage" => Service::Storage {
      host,
      shard: ShardId::new(shard.unwrap_or(0)),
    },
    "load_balancer" => Service::LoadBalancer { host },
    "analytics" => Service::Analytics {
      host,
      shard: ShardId::new(shard.unwrap_or(0)),
    },
    _ => Service::ApiGateway { host }, // default fallback
  }
}

/// Find the next available port starting from the given port
pub async fn find_available_port(start_port: u16) -> Option<u16> {
  for port in start_port ..= start_port + 100 {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    if tokio::net::TcpListener::bind(&addr).await.is_ok() {
      return Some(port);
    }
  }
  None
}

/// Check if a port is available
pub async fn is_port_available(port: u16) -> bool {
  let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
  tokio::net::TcpListener::bind(&addr).await.is_ok()
}

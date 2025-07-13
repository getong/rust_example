//! Member types and service definitions
//!
//! Based on Stract's architecture, members register different service types
//! with chitchat for cluster membership and discovery.

use std::{collections::HashMap, fmt, net::SocketAddr};

use serde::{Deserialize, Serialize};

pub type ShardId = u32;
pub type NodeId = u32;

/// Service types that can be registered with chitchat
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Service {
  /// DHT service for distributed hash table operations
  Dht { host: SocketAddr, shard: ShardId },
  /// General API service
  Api { host: SocketAddr },
  /// Searcher service for search operations
  Searcher { host: SocketAddr, shard: ShardId },
  /// Webgraph service for web graph operations
  Webgraph { host: SocketAddr, shard: ShardId },
}

impl Service {
  /// Get the host address of the service
  pub fn host(&self) -> SocketAddr {
    match self {
      Service::Dht { host, .. } => *host,
      Service::Api { host } => *host,
      Service::Searcher { host, .. } => *host,
      Service::Webgraph { host, .. } => *host,
    }
  }

  /// Get the shard ID if applicable
  pub fn shard(&self) -> Option<ShardId> {
    match self {
      Service::Dht { shard, .. } => Some(*shard),
      Service::Api { .. } => None,
      Service::Searcher { shard, .. } => Some(*shard),
      Service::Webgraph { shard, .. } => Some(*shard),
    }
  }

  /// Check if this is a DHT service
  pub fn is_dht(&self) -> bool {
    matches!(self, Service::Dht { .. })
  }

  /// Check if this is an API service
  pub fn is_api(&self) -> bool {
    matches!(self, Service::Api { .. })
  }

  /// Check if this is a Searcher service
  pub fn is_searcher(&self) -> bool {
    matches!(self, Service::Searcher { .. })
  }

  /// Check if this is a Webgraph service
  pub fn is_webgraph(&self) -> bool {
    matches!(self, Service::Webgraph { .. })
  }
}

impl fmt::Display for Service {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Service::Dht { host, shard } => write!(f, "DHT({}#{})", host, shard),
      Service::Api { host } => write!(f, "API({})", host),
      Service::Searcher { host, shard } => write!(f, "Searcher({}#{})", host, shard),
      Service::Webgraph { host, shard } => write!(f, "Webgraph({}#{})", host, shard),
    }
  }
}

/// Member information for the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
  pub id: NodeId,
  pub service: Service,
  pub ready: bool,
}

impl Member {
  pub fn new(id: NodeId, service: Service) -> Self {
    Self {
      id,
      service,
      ready: false,
    }
  }

  pub fn set_ready(&mut self) {
    self.ready = true;
  }

  pub fn is_ready(&self) -> bool {
    self.ready
  }
}

/// Member registry for tracking cluster members
pub type MemberRegistry = HashMap<NodeId, Member>;

/// Helper functions for working with members
pub mod helpers {
  use super::*;

  /// Get all DHT members from the registry
  pub fn get_dht_members(registry: &MemberRegistry) -> Vec<&Member> {
    registry
      .values()
      .filter(|member| member.service.is_dht())
      .collect()
  }

  /// Get DHT members by shard
  pub fn get_dht_shard_members(registry: &MemberRegistry, shard: ShardId) -> Vec<&Member> {
    registry
      .values()
      .filter(|member| {
        if let Service::Dht { shard: s, .. } = member.service {
          s == shard
        } else {
          false
        }
      })
      .collect()
  }

  /// Get all API members from the registry
  pub fn get_api_members(registry: &MemberRegistry) -> Vec<&Member> {
    registry
      .values()
      .filter(|member| member.service.is_api())
      .collect()
  }

  /// Get ready members only
  pub fn get_ready_members(registry: &MemberRegistry) -> Vec<&Member> {
    registry
      .values()
      .filter(|member| member.is_ready())
      .collect()
  }
}

#[cfg(test)]
mod tests {
  use std::net::{IpAddr, Ipv4Addr};

  use super::*;

  #[test]
  fn test_service_types() {
    let dht_service = Service::Dht {
      host: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
      shard: 0,
    };

    assert!(dht_service.is_dht());
    assert_eq!(dht_service.shard(), Some(0));

    let api_service = Service::Api {
      host: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
    };

    assert!(api_service.is_api());
    assert_eq!(api_service.shard(), None);
  }

  #[test]
  fn test_member_lifecycle() {
    let mut member = Member::new(
      1,
      Service::Api {
        host: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
      },
    );

    assert!(!member.is_ready());
    member.set_ready();
    assert!(member.is_ready());
  }
}

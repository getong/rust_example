use std::net::SocketAddr;

use schemars::JsonSchema;

#[derive(
  serde::Serialize,
  serde::Deserialize,
  PartialEq,
  Eq,
  Hash,
  Clone,
  Copy,
  Debug,
  PartialOrd,
  Ord,
  JsonSchema,
)]
pub struct ShardId(u64);

impl std::fmt::Display for ShardId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "ShardId({})", self.0)
  }
}

impl ShardId {
  pub fn new(id: u64) -> Self {
    Self(id)
  }

  pub fn as_u64(&self) -> u64 {
    self.0
  }
}

impl From<u64> for ShardId {
  fn from(id: u64) -> Self {
    Self(id)
  }
}

impl From<ShardId> for u64 {
  fn from(id: ShardId) -> u64 {
    id.0
  }
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, Clone, Debug, JsonSchema)]
pub enum Service {
  Searcher { host: SocketAddr, shard: ShardId },
  ApiGateway { host: SocketAddr },
  DataProcessor { host: SocketAddr, shard: ShardId },
  Storage { host: SocketAddr, shard: ShardId },
  LoadBalancer { host: SocketAddr },
  Analytics { host: SocketAddr, shard: ShardId },
}

impl std::fmt::Display for Service {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Searcher { host, shard } => write!(f, "Searcher {} {}", host, shard),
      Self::ApiGateway { host } => write!(f, "ApiGateway {}", host),
      Self::DataProcessor { host, shard } => write!(f, "DataProcessor {} {}", host, shard),
      Self::Storage { host, shard } => write!(f, "Storage {} {}", host, shard),
      Self::LoadBalancer { host } => write!(f, "LoadBalancer {}", host),
      Self::Analytics { host, shard } => write!(f, "Analytics {} {}", host, shard),
    }
  }
}

impl Service {
  pub fn host(&self) -> SocketAddr {
    match self {
      Self::Searcher { host, .. } => *host,
      Self::ApiGateway { host } => *host,
      Self::DataProcessor { host, .. } => *host,
      Self::Storage { host, .. } => *host,
      Self::LoadBalancer { host } => *host,
      Self::Analytics { host, .. } => *host,
    }
  }

  pub fn shard(&self) -> Option<ShardId> {
    match self {
      Self::Searcher { shard, .. } => Some(*shard),
      Self::ApiGateway { .. } => None,
      Self::DataProcessor { shard, .. } => Some(*shard),
      Self::Storage { shard, .. } => Some(*shard),
      Self::LoadBalancer { .. } => None,
      Self::Analytics { shard, .. } => Some(*shard),
    }
  }
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, serde::Serialize, serde::Deserialize, JsonSchema)]
pub struct Member {
  pub id: String,
  pub service: Service,
}

impl Member {
  pub fn new(service: Service) -> Self {
    let id = uuid::Uuid::new_v4().to_string();
    Self { id, service }
  }

  pub fn with_id(id: String, service: Service) -> Self {
    Self { id, service }
  }
}

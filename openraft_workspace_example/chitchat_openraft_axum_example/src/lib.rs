use std::collections::HashMap;

use chitchat::{ChitchatId, ClusterStateSnapshot};
use serde::{Deserialize, Serialize};

// Re-export modules
// Core modules that work
pub mod consistency_router;
pub mod distributed;
pub mod core;

// Simplified OpenRaft modules that follow the working example pattern
// Temporarily commented out due to OpenRaft API compatibility issues
// pub mod raft_simple_types;
// pub mod raft_simple_store;
// pub mod raft_simple_network;

// Re-export commonly used types
pub use consistency_router::{ConsistencyLevel, ConsistencyRouter};
// OpenRaft re-exports - temporarily commented out
// pub use raft_simple_network::Router as RaftRouter;
// pub use raft_simple_store::StateMachineStore;
// pub use raft_simple_types::{NodeId, Request as RaftRequest, Response as RaftResponse,
// TypeConfig};

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse {
  pub cluster_id: String,
  pub cluster_state: ClusterStateSnapshot,
  pub live_nodes: Vec<ChitchatId>,
  pub dead_nodes: Vec<ChitchatId>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetKeyValueResponse {
  pub status: bool,
}

/// Enhanced API response that includes both Chitchat and Raft information
#[derive(Debug, Serialize, Deserialize)]
pub struct HybridApiResponse {
  pub cluster_id: String,
  pub chitchat_state: ClusterStateSnapshot,
  pub raft_state: HashMap<String, String>,
  pub live_nodes: Vec<ChitchatId>,
  pub dead_nodes: Vec<ChitchatId>,
  pub consistency_stats: ConsistencyStats,
}

/// Statistics about operations performed on each backend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConsistencyStats {
  pub chitchat_operations: u64,
  pub raft_operations: u64,
  pub hybrid_operations: u64,
  pub total_operations: u64,
}

impl Default for ConsistencyStats {
  fn default() -> Self {
    Self {
      chitchat_operations: 0,
      raft_operations: 0,
      hybrid_operations: 0,
      total_operations: 0,
    }
  }
}

impl ConsistencyStats {
  pub fn new() -> Self {
    Self {
      chitchat_operations: 0,
      raft_operations: 0,
      hybrid_operations: 0,
      total_operations: 0,
    }
  }

  pub fn increment_chitchat(&mut self) {
    self.chitchat_operations += 1;
    self.total_operations += 1;
  }

  pub fn increment_raft(&mut self) {
    self.raft_operations += 1;
    self.total_operations += 1;
  }

  pub fn increment_hybrid(&mut self) {
    self.hybrid_operations += 1;
    self.total_operations += 1;
  }
}

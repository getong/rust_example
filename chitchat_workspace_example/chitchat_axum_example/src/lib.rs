use chitchat::{ChitchatId, ClusterStateSnapshot};
use serde::{Deserialize, Serialize};

pub mod distributed;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterMembersResponse {
  pub members: Vec<distributed::Member>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceUpdateResponse {
  pub status: bool,
  pub message: String,
}

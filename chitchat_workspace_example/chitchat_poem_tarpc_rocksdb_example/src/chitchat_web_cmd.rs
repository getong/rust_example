use chitchat::{ChitchatId, ClusterStateSnapshot};

use crate::common::ChitchatApi;
use poem_openapi::param::Query;
use poem_openapi::payload::Json;
use poem_openapi::OpenApi;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexResponse {
  pub cluster_id: String,
  pub cluster_state: ClusterStateSnapshot,
  pub live_nodes: Vec<ChitchatId>,
  pub dead_nodes: Vec<ChitchatId>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetKeyValueResponse {
  pub status: bool,
}

#[OpenApi]
impl ChitchatApi {
  /// Chitchat state
  #[oai(path = "/", method = "get")]
  async fn index(&self) -> Json<serde_json::Value> {
    let chitchat_guard = self.chitchat.lock().await;
    let response = IndexResponse {
      cluster_id: chitchat_guard.cluster_id().to_string(),
      cluster_state: chitchat_guard.state_snapshot(),
      live_nodes: chitchat_guard.live_nodes().cloned().collect::<Vec<_>>(),
      dead_nodes: chitchat_guard.dead_nodes().cloned().collect::<Vec<_>>(),
    };
    Json(serde_json::to_value(&response).unwrap())
  }

  /// Sets a key-value pair on this node (without validation).
  #[oai(path = "/set_kv/", method = "get")]
  async fn set_kv(&self, key: Query<String>, value: Query<String>) -> Json<serde_json::Value> {
    let mut chitchat_guard = self.chitchat.lock().await;

    let cc_state = chitchat_guard.self_node_state();
    cc_state.set(key.as_str(), value.as_str());

    Json(serde_json::to_value(&SetKeyValueResponse { status: true }).unwrap())
  }
}

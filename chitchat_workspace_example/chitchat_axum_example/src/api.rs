use std::{net::SocketAddr, sync::Arc};

use aide::axum::routing::{get_with, post_with};
use axum::extract::{Json, State};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
  ApiResponse, ClusterMembersResponse, ServiceUpdateResponse, distributed::Cluster,
  utils::create_service,
};

#[derive(Clone)]
pub struct AppState {
  pub cluster: Arc<Cluster>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServiceUpdateParams {
  /// The type of service to update
  pub service_type: String,
  /// The host address for the service
  pub host: String,
  /// Optional shard ID for sharded services
  pub shard: Option<u64>,
}

/// Get the current chitchat cluster state
pub async fn get_state(State(state): State<AppState>) -> Json<ApiResponse> {
  let cluster_state = state.cluster.cluster_state().await;
  let live_nodes = state.cluster.live_nodes().await;
  let dead_nodes = state.cluster.dead_nodes().await;

  let response = ApiResponse {
    cluster_id: "chitchat-example-cluster".to_string(),
    cluster_state: serde_json::to_value(cluster_state).unwrap_or_default(),
    live_nodes: live_nodes.into_iter().map(|id| id.node_id).collect(),
    dead_nodes: dead_nodes.into_iter().map(|id| id.node_id).collect(),
  };
  Json(response)
}

/// Get cluster members with their services
pub async fn get_members(State(state): State<AppState>) -> Json<ClusterMembersResponse> {
  let members = state.cluster.members().await;
  Json(ClusterMembersResponse { members })
}

/// Update the service of the current node
pub async fn update_service(
  State(state): State<AppState>,
  Json(params): Json<ServiceUpdateParams>,
) -> Json<ServiceUpdateResponse> {
  let host: SocketAddr = match params.host.parse() {
    Ok(addr) => addr,
    Err(_) => {
      return Json(ServiceUpdateResponse {
        status: false,
        message: "Invalid host format".to_string(),
      });
    }
  };

  let service = create_service(&params.service_type, host, params.shard);

  match state.cluster.set_service(service).await {
    Ok(_) => Json(ServiceUpdateResponse {
      status: true,
      message: "Service updated successfully".to_string(),
    }),
    Err(e) => Json(ServiceUpdateResponse {
      status: false,
      message: format!("Failed to update service: {}", e),
    }),
  }
}

pub fn get_state_docs() -> impl Into<aide::axum::routing::ApiMethodRouter<AppState>> {
  get_with(get_state, |op| {
    op.summary("Get cluster state")
      .description(
        "Returns the current state of the chitchat cluster including live and dead nodes",
      )
      .response::<200, ApiResponse>()
  })
}

pub fn get_members_docs() -> impl Into<aide::axum::routing::ApiMethodRouter<AppState>> {
  get_with(get_members, |op| {
    op.summary("Get cluster members")
      .description("Returns all members in the cluster with their service information")
      .response::<200, ClusterMembersResponse>()
  })
}

pub fn update_service_docs() -> impl Into<aide::axum::routing::ApiMethodRouter<AppState>> {
  post_with(update_service, |op| {
    op.summary("Update service")
      .description("Updates the service configuration for the current node")
      .response::<200, ServiceUpdateResponse>()
  })
}

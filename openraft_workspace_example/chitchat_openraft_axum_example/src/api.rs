use std::{net::SocketAddr, sync::Arc};

use aide::axum::routing::{get_with, post_with};
use axum::extract::{Json, Path, State};
use base64::prelude::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
  ApiResponse, ClusterMembersResponse, ServiceUpdateResponse,
  distributed::{
    Cluster,
    raft_types::{Key, Request, Response, Table, Value},
  },
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

/// Parameters for OpenRAFT key-value operations
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RaftSetRequest {
  /// The table name
  pub table: String,
  /// The key to set
  pub key: String,
  /// The value to set (base64 encoded)
  pub value: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RaftGetRequest {
  /// The table name
  pub table: String,
  /// The key to get
  pub key: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RaftResponse {
  /// Success status
  pub success: bool,
  /// Response message
  pub message: String,
  /// Optional data payload
  pub data: Option<serde_json::Value>,
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

/// Set a key-value pair using OpenRAFT
pub async fn raft_set(
  State(state): State<AppState>,
  Json(params): Json<RaftSetRequest>,
) -> Json<RaftResponse> {
  let value_bytes = match base64::prelude::BASE64_STANDARD.decode(&params.value) {
    Ok(bytes) => bytes,
    Err(_) => {
      return Json(RaftResponse {
        success: false,
        message: "Invalid base64 value".to_string(),
        data: None,
      });
    }
  };

  let request = Request::Set {
    table: Table(params.table),
    key: Key(params.key),
    value: Value(value_bytes),
  };

  match state.cluster.raft_request(request).await {
    Ok(Response::Set(Ok(()))) => Json(RaftResponse {
      success: true,
      message: "Key set successfully".to_string(),
      data: None,
    }),
    Ok(Response::Set(Err(e))) => Json(RaftResponse {
      success: false,
      message: format!("Failed to set key: {}", e),
      data: None,
    }),
    Ok(_) => Json(RaftResponse {
      success: false,
      message: "Unexpected response type".to_string(),
      data: None,
    }),
    Err(e) => Json(RaftResponse {
      success: false,
      message: format!("Raft request failed: {}", e),
      data: None,
    }),
  }
}

/// Get a value by key using OpenRAFT
pub async fn raft_get(
  State(state): State<AppState>,
  Path((table, key)): Path<(String, String)>,
) -> Json<RaftResponse> {
  let request = Request::Get {
    table: Table(table),
    key: Key(key),
  };

  match state.cluster.raft_request(request).await {
    Ok(Response::Get(Ok(Some(value)))) => {
      let encoded_value = base64::prelude::BASE64_STANDARD.encode(&value.0);
      Json(RaftResponse {
        success: true,
        message: "Key found".to_string(),
        data: Some(serde_json::json!({ "value": encoded_value })),
      })
    }
    Ok(Response::Get(Ok(None))) => Json(RaftResponse {
      success: false,
      message: "Key not found".to_string(),
      data: None,
    }),
    Ok(Response::Get(Err(e))) => Json(RaftResponse {
      success: false,
      message: format!("Failed to get key: {}", e),
      data: None,
    }),
    Ok(_) => Json(RaftResponse {
      success: false,
      message: "Unexpected response type".to_string(),
      data: None,
    }),
    Err(e) => Json(RaftResponse {
      success: false,
      message: format!("Raft request failed: {}", e),
      data: None,
    }),
  }
}

/// List all tables using OpenRAFT
pub async fn raft_list_tables(State(state): State<AppState>) -> Json<RaftResponse> {
  let request = Request::AllTables;

  match state.cluster.raft_request(request).await {
    Ok(Response::AllTables(Ok(tables))) => {
      let table_names: Vec<String> = tables.into_iter().map(|t| t.0).collect();
      Json(RaftResponse {
        success: true,
        message: "Tables listed successfully".to_string(),
        data: Some(serde_json::json!({ "tables": table_names })),
      })
    }
    Ok(Response::AllTables(Err(e))) => Json(RaftResponse {
      success: false,
      message: format!("Failed to list tables: {}", e),
      data: None,
    }),
    Ok(_) => Json(RaftResponse {
      success: false,
      message: "Unexpected response type".to_string(),
      data: None,
    }),
    Err(e) => Json(RaftResponse {
      success: false,
      message: format!("Raft request failed: {}", e),
      data: None,
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

pub fn raft_set_docs() -> impl Into<aide::axum::routing::ApiMethodRouter<AppState>> {
  post_with(raft_set, |op| {
    op.summary("Set key-value using OpenRAFT")
      .description("Sets a key-value pair in the distributed store using OpenRAFT consensus")
      .response::<200, Json<RaftResponse>>()
  })
}

pub fn raft_get_docs() -> impl Into<aide::axum::routing::ApiMethodRouter<AppState>> {
  get_with(raft_get, |op| {
    op.summary("Get value by key using OpenRAFT")
      .description("Retrieves a value by key from the distributed store")
      .response::<200, Json<RaftResponse>>()
  })
}

pub fn raft_list_tables_docs() -> impl Into<aide::axum::routing::ApiMethodRouter<AppState>> {
  get_with(raft_list_tables, |op| {
    op.summary("List all tables using OpenRAFT")
      .description("Lists all tables in the distributed store")
      .response::<200, Json<RaftResponse>>()
  })
}

use std::{net::SocketAddr, sync::Arc};

use axum::{
  extract::{Query, State},
  response::Json,
};
use serde::Deserialize;

use crate::{
  ApiResponse, ClusterMembersResponse, ServiceUpdateResponse,
  distributed::{Cluster, Service, ShardId},
};

#[derive(Clone)]
pub struct AppState {
  pub cluster: Arc<Cluster>,
}

#[derive(Debug, Deserialize)]
pub struct ServiceUpdateParams {
  pub service_type: String,
  pub host: String,
  pub shard: Option<u64>,
}

/// Get the current chitchat cluster state
pub async fn get_state(State(state): State<AppState>) -> Json<ApiResponse> {
  let cluster_state = state.cluster.cluster_state().await;
  let live_nodes = state.cluster.live_nodes().await;
  let dead_nodes = state.cluster.dead_nodes().await;

  let response = ApiResponse {
    cluster_id: "chitchat-example-cluster".to_string(),
    cluster_state,
    live_nodes,
    dead_nodes,
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
  Query(params): Query<ServiceUpdateParams>,
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

  let service = match params.service_type.as_str() {
    "searcher" => {
      let shard = params.shard.unwrap_or(0);
      Service::Searcher {
        host,
        shard: ShardId::new(shard),
      }
    }
    "api_gateway" => Service::ApiGateway { host },
    "data_processor" => {
      let shard = params.shard.unwrap_or(0);
      Service::DataProcessor {
        host,
        shard: ShardId::new(shard),
      }
    }
    "storage" => {
      let shard = params.shard.unwrap_or(0);
      Service::Storage {
        host,
        shard: ShardId::new(shard),
      }
    }
    "load_balancer" => Service::LoadBalancer { host },
    "analytics" => {
      let shard = params.shard.unwrap_or(0);
      Service::Analytics {
        host,
        shard: ShardId::new(shard),
      }
    }
    _ => {
      return Json(ServiceUpdateResponse {
        status: false,
        message: "Unknown service type".to_string(),
      });
    }
  };

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

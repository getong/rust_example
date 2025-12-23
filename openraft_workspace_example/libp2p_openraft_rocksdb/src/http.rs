use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use axum::{
  Json, Router,
  extract::State,
  routing::{get, post},
};
use openraft_rocksstore::RocksRequest;
use serde::{Deserialize, Serialize};

use crate::{
  network::{
    rpc::{RaftRpcRequest, RaftRpcResponse},
    transport::Libp2pNetworkFactory,
  },
  typ::{ClientWriteResponse, NodeId, Raft, RaftMetrics},
};

#[derive(Clone)]
pub struct AppState {
  pub node_id: NodeId,
  pub node_name: String,
  pub peer_id: String,
  pub listen: String,
  pub network: Libp2pNetworkFactory,
  pub raft: Raft,
}

pub async fn serve(addr: SocketAddr, state: AppState) -> anyhow::Result<()> {
  let app = Router::new()
    .route("/cluster", get(cluster_info))
    .route("/write", post(write_value))
    .with_state(Arc::new(state));

  let listener = tokio::net::TcpListener::bind(addr)
    .await
    .context("bind http")?;
  axum::serve(listener, app).await.context("serve http")?;
  Ok(())
}

#[derive(Serialize)]
struct ClusterInfoResponse {
  node_id: NodeId,
  node_name: String,
  peer_id: String,
  listen: String,
  known_nodes: Vec<KnownNodeResponse>,
  raft_metrics: serde_json::Value,
}

#[derive(Serialize)]
struct KnownNodeResponse {
  node_id: NodeId,
  peer_id: String,
  addr: String,
}

#[derive(Deserialize)]
struct WriteValueRequest {
  key: String,
  value: i64,
  target_node_id: Option<NodeId>,
}

#[derive(Serialize)]
struct WriteValueResponse {
  target_node_id: Option<NodeId>,
  result: Result<ClientWriteResponse, String>,
}

async fn cluster_info(State(state): State<Arc<AppState>>) -> Json<ClusterInfoResponse> {
  let mut nodes: Vec<KnownNodeResponse> = state
    .network
    .known_nodes()
    .await
    .into_iter()
    .map(|(node_id, peer_id, addr)| KnownNodeResponse {
      node_id,
      peer_id: peer_id.to_string(),
      addr: addr.to_string(),
    })
    .collect();

  nodes.sort_by_key(|node| node.node_id);

  let metrics = state.raft.metrics().borrow().clone();
  let raft_metrics = serde_json::to_value(metrics)
    .unwrap_or_else(|err| serde_json::Value::String(format!("metrics serialize error: {err}")));

  Json(ClusterInfoResponse {
    node_id: state.node_id,
    node_name: state.node_name.clone(),
    peer_id: state.peer_id.clone(),
    listen: state.listen.clone(),
    known_nodes: nodes,
    raft_metrics,
  })
}

async fn write_value(
  State(state): State<Arc<AppState>>,
  Json(req): Json<WriteValueRequest>,
) -> Json<WriteValueResponse> {
  let metrics = state.raft.metrics().borrow().clone();
  let mut target_node_id = req.target_node_id;
  if target_node_id.is_none() {
    target_node_id = resolve_leader_id(state.as_ref(), &metrics).await;
  }

  let Some(target_node_id) = target_node_id else {
    return Json(WriteValueResponse {
      target_node_id: None,
      result: Err("no leader available".to_string()),
    });
  };

  let request = RocksRequest::Set {
    key: req.key,
    value: req.value.to_string(),
  };

  let mut final_target_id = target_node_id;
  let mut result = send_write(state.as_ref(), final_target_id, request.clone()).await;

  if result.is_err() && final_target_id != state.node_id && req.target_node_id.is_none() {
    if let Some(leader_id) = discover_leader_id(state.as_ref(), Some(final_target_id)).await {
      final_target_id = leader_id;
      result = send_write(state.as_ref(), final_target_id, request).await;
    }
  }

  Json(WriteValueResponse {
    target_node_id: Some(final_target_id),
    result,
  })
}

async fn resolve_leader_id(state: &AppState, metrics: &RaftMetrics) -> Option<NodeId> {
  if metrics.state.is_leader() {
    return Some(state.node_id);
  }

  if let Some(leader_id) = metrics.current_leader {
    return Some(leader_id);
  }

  discover_leader_id(state, None).await
}

async fn discover_leader_id(state: &AppState, skip: Option<NodeId>) -> Option<NodeId> {
  let nodes = state.network.known_nodes().await;
  for (node_id, _peer, _addr) in nodes {
    if node_id == state.node_id || Some(node_id) == skip {
      continue;
    }

    let resp = state
      .network
      .request(node_id, RaftRpcRequest::GetMetrics)
      .await;

    let Ok(RaftRpcResponse::GetMetrics(metrics)) = resp else {
      continue;
    };

    if metrics.state.is_leader() {
      return Some(node_id);
    }
    if let Some(leader_id) = metrics.current_leader {
      return Some(leader_id);
    }
  }
  None
}

async fn send_write(
  state: &AppState,
  target_node_id: NodeId,
  request: RocksRequest,
) -> Result<ClientWriteResponse, String> {
  if target_node_id == state.node_id {
    state
      .raft
      .client_write(request)
      .await
      .map_err(|err| format!("{err:?}"))
  } else {
    match state
      .network
      .request(target_node_id, RaftRpcRequest::ClientWrite(request))
      .await
    {
      Ok(RaftRpcResponse::ClientWrite(res)) => res.map_err(|err| format!("{err:?}")),
      Ok(RaftRpcResponse::Error(err)) => Err(err),
      Ok(other) => Err(format!("unexpected response: {other:?}")),
      Err(err) => Err(format!("libp2p error: {err}")),
    }
  }
}

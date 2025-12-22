use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;

use crate::{
  network::transport::Libp2pNetworkFactory,
  typ::{NodeId, Raft},
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

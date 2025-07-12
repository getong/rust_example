//! This module contains APIs for cluster management such as joining and leaving nodes.

use std::{
  collections::{BTreeMap, BTreeSet},
  net::SocketAddr,
};

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use openraft::BasicNode;

use crate::node::{NodeId, RaftApp};

#[tracing::instrument(level = "debug", skip(app_state))]
pub async fn init(State(app_state): State<RaftApp>) -> impl IntoResponse {
  let mut nodes = BTreeMap::new();
  nodes.insert(
    app_state.id,
    BasicNode {
      addr: app_state.bind_addr.to_string(),
    },
  );
  let res = app_state.raft.initialize(nodes).await;
  (StatusCode::CREATED, Json(res))
}

#[tracing::instrument(level = "debug", skip(app_state))]
pub async fn add_learner(
  State(app_state): State<RaftApp>,
  Json(req): Json<(NodeId, SocketAddr)>,
) -> impl IntoResponse {
  let node_id = req.0;
  let node = BasicNode {
    addr: req.1.to_string(),
  };
  let res = app_state.raft.add_learner(node_id, node, true).await;
  (StatusCode::OK, Json(res))
}

#[tracing::instrument(level = "debug", skip(app_state))]
pub async fn change_membership(
  State(app_state): State<RaftApp>,
  Json(req): Json<BTreeSet<NodeId>>,
) -> impl IntoResponse {
  let res = app_state.raft.change_membership(req, true).await;
  (StatusCode::OK, Json(res))
}

#[tracing::instrument(level = "debug", skip(app_state))]
pub async fn metrics(State(app_state): State<RaftApp>) -> impl IntoResponse {
  let metrics = app_state.raft.metrics().borrow().clone();

  let res: Result<_, openraft::error::Infallible> = Ok(metrics);
  (StatusCode::OK, Json(res))
}

#[tracing::instrument(level = "debug", skip(app_state))]
pub async fn get_id(State(app_state): State<RaftApp>) -> impl IntoResponse {
  (StatusCode::CREATED, Json(app_state.id))
}

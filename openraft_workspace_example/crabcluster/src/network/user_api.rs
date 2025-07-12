//! Module containing user-centric APIs.

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use openraft::error::Infallible;

use crate::{node::RaftApp, store::RaftRequest};

#[tracing::instrument(level = "debug", skip(app_state, req))]
pub async fn kv_write(
  State(app_state): State<RaftApp>,
  Json(req): Json<RaftRequest>,
) -> impl IntoResponse {
  let res = app_state.raft.client_write(req).await;
  (StatusCode::OK, Json(res))
}

#[tracing::instrument(level = "debug", skip(app_state, req))]
pub async fn kv_read(
  State(app_state): State<RaftApp>,
  Json(req): Json<String>,
) -> impl IntoResponse {
  let state_machine = app_state.store.state_machine.read().await;
  let key = req;
  let value = state_machine.data.get(&key).cloned();

  let res: Result<String, Infallible> = Ok(value.unwrap_or_default());
  (StatusCode::OK, Json(res))
}

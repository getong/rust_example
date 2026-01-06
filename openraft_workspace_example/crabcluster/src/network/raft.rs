//! Module containing Raft-specific operations

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use openraft::raft::{AppendEntriesRequest, InstallSnapshotRequest, VoteRequest};
use openraft_legacy::network_v1::ChunkedSnapshotReceiver;

use crate::node::{RaftApp, RaftTypeConfig};

#[tracing::instrument(level = "trace", skip(app_state))]
pub async fn append(
  State(app_state): State<RaftApp>,
  Json(req): Json<AppendEntriesRequest<RaftTypeConfig>>,
) -> impl IntoResponse {
  let res = app_state.raft.append_entries(req).await;
  (StatusCode::CREATED, Json(res))
}

#[tracing::instrument(level = "trace", skip(app_state))]
pub async fn snapshot(
  State(app_state): State<RaftApp>,
  Json(req): Json<InstallSnapshotRequest<RaftTypeConfig>>,
) -> impl IntoResponse {
  let res = app_state.raft.install_snapshot(req).await;
  (StatusCode::CREATED, Json(res))
}

#[tracing::instrument(level = "trace", skip(app_state))]
pub async fn vote(
  State(app_state): State<RaftApp>,
  Json(req): Json<VoteRequest<RaftTypeConfig>>,
) -> impl IntoResponse {
  let res = app_state.raft.vote(req).await;
  (StatusCode::CREATED, Json(res))
}

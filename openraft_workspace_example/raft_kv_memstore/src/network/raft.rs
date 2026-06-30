use std::sync::Arc;

use axum::{Json, extract::State, response::IntoResponse};
use openraft::{
  Snapshot,
  errors::RaftError,
  raft::{AppendEntriesRequest, TransferLeaderRequest, TransferLeaderResponse, VoteRequest},
};

use crate::{
  TypeConfig,
  app::App,
  typ::{SnapshotMeta, SnapshotResponse, Vote},
};

// --- Raft communication

pub async fn vote(
  State(app): State<Arc<App>>,
  req: Json<VoteRequest<TypeConfig>>,
) -> impl IntoResponse {
  let res = app.raft.vote(req.0).await;
  Json(res)
}

pub async fn append(
  State(app): State<Arc<App>>,
  req: Json<AppendEntriesRequest<TypeConfig>>,
) -> impl IntoResponse {
  let res = app.raft.append_entries(req.0).await;
  Json(res)
}

pub async fn snapshot(
  State(app): State<Arc<App>>,
  req: Json<(Vote, SnapshotMeta, Vec<u8>)>,
) -> impl IntoResponse {
  let (snapshot_vote, meta, data) = req.0;
  let snapshot = Snapshot {
    meta,
    snapshot: std::io::Cursor::new(data),
  };
  let res: Result<SnapshotResponse, RaftError<TypeConfig>> = app
    .raft
    .install_full_snapshot(snapshot_vote, snapshot)
    .await
    .map_err(RaftError::Fatal);
  Json(res)
}

pub async fn transfer_leader(
  State(app): State<Arc<App>>,
  req: Json<TransferLeaderRequest<TypeConfig>>,
) -> impl IntoResponse {
  let res: Result<TransferLeaderResponse<TypeConfig>, RaftError<TypeConfig>> = app
    .raft
    .handle_transfer_leader(req.0)
    .await
    .map_err(RaftError::Fatal);
  Json(res)
}

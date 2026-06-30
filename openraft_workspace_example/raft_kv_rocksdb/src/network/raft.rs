use actix_web::{
  Responder, post,
  web::{Data, Json},
};
use openraft::{
  Snapshot,
  errors::RaftError,
  raft::{TransferLeaderRequest, TransferLeaderResponse},
};

use crate::{app::App, typ::*};

// --- Raft communication

#[post("/vote")]
pub async fn vote(app: Data<App>, req: Json<VoteRequest>) -> actix_web::Result<impl Responder> {
  let res = app.raft.vote(req.0).await;
  Ok(Json(res))
}

#[post("/append")]
pub async fn append(
  app: Data<App>,
  req: Json<AppendEntriesRequest>,
) -> actix_web::Result<impl Responder> {
  let res = app.raft.append_entries(req.0).await;
  Ok(Json(res))
}

#[post("/snapshot")]
pub async fn snapshot(
  app: Data<App>,
  req: Json<(Vote, SnapshotMeta, Vec<u8>)>,
) -> actix_web::Result<impl Responder> {
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
  Ok(Json(res))
}

#[post("/transfer-leader")]
pub async fn transfer_leader(
  app: Data<App>,
  req: Json<TransferLeaderRequest<TypeConfig>>,
) -> actix_web::Result<impl Responder> {
  let res: Result<TransferLeaderResponse<TypeConfig>, RaftError<TypeConfig>> = app
    .raft
    .handle_transfer_leader(req.0)
    .await
    .map_err(RaftError::Fatal);
  Ok(Json(res))
}

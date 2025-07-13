use actix_web::{
  post,
  web::{Data, Json},
  Responder,
};
use openraft::raft::{AppendEntriesRequest, InstallSnapshotRequest, VoteRequest};

use crate::{app::App, TypeConfig};

// --- Raft communication

#[post("/vote")]
pub async fn vote(
  app: Data<App>,
  req: Json<VoteRequest<TypeConfig>>,
) -> actix_web::Result<impl Responder> {
  let res = app.raft.vote(req.0).await;
  Ok(Json(res))
}

#[post("/append")]
pub async fn append(
  app: Data<App>,
  req: Json<AppendEntriesRequest<TypeConfig>>,
) -> actix_web::Result<impl Responder> {
  let res = app.raft.append_entries(req.0).await;
  Ok(Json(res))
}

#[post("/snapshot")]
pub async fn snapshot(
  app: Data<App>,
  req: Json<InstallSnapshotRequest<TypeConfig>>,
) -> actix_web::Result<impl Responder> {
  let res = app.raft.install_snapshot(req.0).await;
  Ok(Json(res))
}

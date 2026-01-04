use actix_web::{
  Responder, post,
  web::{Data, Json},
};
use openraft::error::decompose::DecomposeResult;
use openraft_legacy::prelude::*;

use crate::{app::App, typ::*};

// --- Raft communication

#[post("/vote")]
pub async fn vote(app: Data<App>, req: Json<VoteRequest>) -> actix_web::Result<impl Responder> {
  let res = app.raft.vote(req.0).await.decompose().unwrap();
  Ok(Json(res))
}

#[post("/append")]
pub async fn append(
  app: Data<App>,
  req: Json<AppendEntriesRequest>,
) -> actix_web::Result<impl Responder> {
  let res = app.raft.append_entries(req.0).await.decompose().unwrap();
  Ok(Json(res))
}

#[post("/snapshot")]
pub async fn snapshot(
  app: Data<App>,
  req: Json<InstallSnapshotRequest>,
) -> actix_web::Result<impl Responder> {
  let res = app.raft.install_snapshot(req.0).await;
  Ok(Json(res))
}

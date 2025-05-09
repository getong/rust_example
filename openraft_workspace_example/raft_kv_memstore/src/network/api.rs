use actix_web::{post, web, web::Data, Responder};
use openraft::error::{CheckIsLeaderError, Infallible, RaftError};
use web::Json;

use crate::{app::App, store::Request, TypeConfig};

/// Application API
///
/// This is where you place your application, you can use the example below to create your
/// API. The current implementation:
///
///  - `POST - /write` saves a value in a key and sync the nodes.
///  - `POST - /read` attempt to find a value from a given key.
#[post("/write")]
pub async fn write(app: Data<App>, req: Json<Request>) -> actix_web::Result<impl Responder> {
  let response = app.raft.client_write(req.0).await;
  Ok(Json(response))
}

#[post("/read")]
pub async fn read(app: Data<App>, req: Json<String>) -> actix_web::Result<impl Responder> {
  let state_machine = app.state_machine_store.state_machine.read().await;
  let key = req.0;
  let value = state_machine.data.get(&key).cloned();

  let res: Result<String, Infallible> = Ok(value.unwrap_or_default());
  Ok(Json(res))
}

#[post("/linearizable_read")]
pub async fn linearizable_read(
  app: Data<App>,
  req: Json<String>,
) -> actix_web::Result<impl Responder> {
  let ret = app.raft.ensure_linearizable().await;

  match ret {
    Ok(_) => {
      let state_machine = app.state_machine_store.state_machine.read().await;
      let key = req.0;
      let value = state_machine.data.get(&key).cloned();

      let res: Result<String, RaftError<TypeConfig, CheckIsLeaderError<TypeConfig>>> =
        Ok(value.unwrap_or_default());
      Ok(Json(res))
    }
    Err(e) => Ok(Json(Err(e))),
  }
}

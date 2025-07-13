use actix_web::{post, web, web::Data, Responder};
use openraft::{
  error::{decompose::DecomposeResult, CheckIsLeaderError, Infallible},
  ReadPolicy,
};
use web::Json;

use crate::{app::App, store::Request, TypeConfig};

#[post("/write")]
pub async fn write(app: Data<App>, req: Json<Request>) -> actix_web::Result<impl Responder> {
  let response = app.raft.client_write(req.0).await.decompose().unwrap();
  Ok(Json(response))
}

#[post("/read")]
pub async fn read(app: Data<App>, req: Json<String>) -> actix_web::Result<impl Responder> {
  let key = req.0;
  let kvs = app.key_values.read().await;
  let value = kvs.get(&key);

  let res: Result<String, Infallible> = Ok(value.cloned().unwrap_or_default());
  Ok(Json(res))
}

#[post("/linearizable_read")]
pub async fn linearizable_read(
  app: Data<App>,
  req: Json<String>,
) -> actix_web::Result<impl Responder> {
  let ret = app
    .raft
    .get_read_linearizer(ReadPolicy::ReadIndex)
    .await
    .decompose()
    .unwrap();

  match ret {
    Ok(linearizer) => {
      linearizer.await_ready(&app.raft).await.unwrap();

      let key = req.0;
      let kvs = app.key_values.read().await;
      let value = kvs.get(&key);

      let res: Result<String, CheckIsLeaderError<TypeConfig>> =
        Ok(value.cloned().unwrap_or_default());
      Ok(Json(res))
    }
    Err(e) => Ok(Json(Err(e))),
  }
}

use std::sync::Arc;

use axum::{Json, extract::State, response::IntoResponse};
use openraft::{
  ReadPolicy,
  error::{Infallible, LinearizableReadError, decompose::DecomposeResult},
};

use crate::{TypeConfig, app::App};

pub async fn write(State(app): State<Arc<App>>, req: Json<types_kv::Request>) -> impl IntoResponse {
  let response = app.raft.client_write(req.0).await.decompose().unwrap();
  Json(response)
}

pub async fn read(State(app): State<Arc<App>>, req: Json<String>) -> impl IntoResponse {
  let key = req.0;
  let kvs = app.key_values.lock().await;
  let value = kvs.get(&key);

  let res: Result<String, Infallible> = Ok(value.cloned().unwrap_or_default());
  Json(res)
}

pub async fn linearizable_read(
  State(app): State<Arc<App>>,
  req: Json<String>,
) -> impl IntoResponse {
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
      let kvs = app.key_values.lock().await;
      let value = kvs.get(&key);

      let res: Result<String, LinearizableReadError<TypeConfig>> =
        Ok(value.cloned().unwrap_or_default());
      Json(res)
    }
    Err(e) => Json(Err(e)),
  }
}

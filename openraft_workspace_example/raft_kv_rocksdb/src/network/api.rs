use std::sync::Arc;

use openraft::error::Infallible;
use tide::{Body, Request, Response, StatusCode};

use crate::{app::App, typ::*, Server};

pub fn rest(app: &mut Server) {
  let mut api = app.at("/api");
  api.at("/write").post(write);
  api.at("/read").post(read);
  api.at("/linearizable_read").post(linearizable_read);
}
/// Application API
///
/// This is where you place your application, you can use the example below to create your
/// API. The current implementation:
///
///  - `POST - /write` saves a value in a key and sync the nodes.
///  - `POST - /read` attempt to find a value from a given key.
async fn write(mut req: Request<Arc<App>>) -> tide::Result {
  let body = req.body_json().await?;
  let res = req.state().raft.client_write(body).await;
  Ok(
    Response::builder(StatusCode::Ok)
      .body(Body::from_json(&res)?)
      .build(),
  )
}

async fn read(mut req: Request<Arc<App>>) -> tide::Result {
  let key: String = req.body_json().await?;
  let kvs = req.state().key_values.read().await;
  let value = kvs.get(&key);

  let res: Result<String, Infallible> = Ok(value.cloned().unwrap_or_default());
  Ok(
    Response::builder(StatusCode::Ok)
      .body(Body::from_json(&res)?)
      .build(),
  )
}

async fn linearizable_read(mut req: Request<Arc<App>>) -> tide::Result {
  let ret = req.state().raft.ensure_linearizable().await;

  match ret {
    Ok(_) => {
      let key: String = req.body_json().await?;
      let kvs = req.state().key_values.read().await;

      let value = kvs.get(&key);

      let res: Result<String, CheckIsLeaderError> = Ok(value.cloned().unwrap_or_default());
      Ok(
        Response::builder(StatusCode::Ok)
          .body(Body::from_json(&res)?)
          .build(),
      )
    }
    e => Ok(
      Response::builder(StatusCode::Ok)
        .body(Body::from_json(&e)?)
        .build(),
    ),
  }
}

use crate::common::Api;
use crate::Node;
use crate::Request;
use openraft::error::CheckIsLeaderError;
use poem_openapi::{payload::Json, ApiResponse, OpenApi};
#[derive(ApiResponse)]
pub enum SearchResponse {
  #[oai(status = 200)]
  Ok(Json<String>),
}

#[derive(ApiResponse)]
pub enum WriteResponse {
  #[oai(status = 200)]
  Ok(Json<String>),
}

#[derive(ApiResponse)]
pub enum ConsistentReadResponse {
  #[oai(status = 200)]
  Ok(Json<String>),
  #[oai(status = 500)]
  Fail,
}

#[OpenApi]
impl Api {
  #[oai(path = "/read", method = "post")]
  pub async fn read(&self, name: Json<String>) -> SearchResponse {
    let state_machine = self.store.state_machine.read().await;
    let value = state_machine
      .get(&name.0)
      .unwrap_or_default()
      .unwrap_or_default();

    SearchResponse::Ok(Json(value))
  }

  #[oai(path = "/write", method = "post")]
  pub async fn write(&self, name: Json<String>) -> WriteResponse {
    let req = Request::Set {
      key: name.0.clone(),
      value: name.0,
    };
    let result = self.raft.client_write(req).await;
    match result {
      Ok(_) => WriteResponse::Ok(Json("ok".to_string())),
      _ => WriteResponse::Ok(Json("failed".to_string())),
    }
  }

  #[oai(path = "/consistent_read", method = "post")]
  pub async fn consistent_read(&self, name: Json<String>) -> ConsistentReadResponse {
    let ret = self.raft.ensure_linearizable().await;

    match ret {
      Ok(_) => {
        let state_machine = self.store.state_machine.read().await;

        let value = state_machine.get(&name.0).unwrap_or_default();

        let res: Result<String, CheckIsLeaderError<u64, Node>> = Ok(value.unwrap_or_default());
        match res {
          Ok(result) => ConsistentReadResponse::Ok(Json(result)),
          Err(_) => ConsistentReadResponse::Fail,
        }
      }
      _e => ConsistentReadResponse::Fail,
    }
  }
}

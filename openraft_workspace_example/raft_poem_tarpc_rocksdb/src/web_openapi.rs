use crate::common::Api;
use poem_openapi::{param::Query, payload::PlainText};
use poem_openapi::{payload::Json, ApiResponse, OpenApi};

#[derive(ApiResponse)]
pub enum SearchResponse {
  #[oai(status = 200)]
  Ok(Json<String>),
}

#[OpenApi]
impl Api {
  // #[oai(path = "/hello", method = "get")]
  // pub async fn index(&self, name: Query<Option<String>>) -> PlainText<String> {
  //   let recv_name = match name.0 {
  //     Some(name) => name,
  //     None => "unknown!".to_string(),
  //   };
  //   PlainText(format!(
  //     "hello, {}, the current num is {:?}!\n",
  //     recv_name,
  //     self.num.lock().await
  //   ))
  // }

  #[oai(path = "/read", method = "post")]
  pub async fn write(&self, name: Json<String>) -> SearchResponse {
    let state_machine = self.store.state_machine.read().await;
    let value = state_machine
      .get(&name.0)
      .unwrap_or_default()
      .unwrap_or_default();

    SearchResponse::Ok(Json(value))
  }
}

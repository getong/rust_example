use axum::{
  extract::{Path, Query},
  routing::post,
  Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

// work with ../../../reqwest_workspace_example/reqwest_send_basic_auth_example/src
#[derive(Serialize, Deserialize)]
pub struct MyStruct {
  a: String,
  b: Option<String>,
  c: Option<String>,
  d: Option<String>,
}

pub async fn state(
  Path(channel_id): Path<String>,
  Query(params): Query<MyStruct>,
  auth: String,
) -> impl axum::response::IntoResponse {
  // Your handler logic here
  println!("auth is {}", auth);
  Json(json!({ "channel_id": channel_id, "params": params, "auth": auth }))
}

#[tokio::main]
async fn main() {
  let app = Router::new().route("/state/:channel", post(state));

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
  axum::serve(listener, app).await.unwrap();
}

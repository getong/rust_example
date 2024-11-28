use std::collections::HashMap;

use axum::{
  extract::{Path, Query},
  routing::{get, post},
  Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Serialize, Deserialize, Debug)]
pub struct Params {
  pub apikey: String,
  pub channel: Option<String>,
  pub ep_name: Option<String>,
  pub block: Option<String>,
}

// work with ../../../reqwest_workspace_example/reqwest_handle_axum_query_params_example/
pub async fn sign(Path(project): Path<String>, Query(params): Query<Params>) -> Json<Value> {
  // Your handler logic here
  println!("params: {:?}", params);
  Json(json!({ "project": project, "params": params }))
}

// curl "http://localhost:3000/hello?data=2"
// curl "http://localhost:3000/hello?hello=2"
async fn query(Query(params): Query<HashMap<String, String>>) -> String {
  format!(
    "Hello, {}",
    params.get("hello").unwrap_or(&"world".to_string())
  )
}

#[tokio::main]
async fn main() {
  let router = Router::new()
    .route("/hello", get(query))
    .route("/sign/:project", post(sign));

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  axum::serve(listener, router).await.unwrap();
}

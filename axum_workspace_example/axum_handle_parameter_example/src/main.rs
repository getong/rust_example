use axum::{routing::get, Router};

use axum::extract::Query;
use std::collections::HashMap;

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
  let router = Router::new().route("/hello", get(query));

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  axum::serve(listener, router).await.unwrap();
}

use axum::response::IntoResponse;
use axum::routing::*;
use axum::Router;

use axum_streams::*;
use futures::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio_stream::StreamExt;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct MyTestStructure {
  some_test_field: String,
}

fn source_test_stream() -> impl Stream<Item = MyTestStructure> {
  // Simulating a stream with a plain vector and throttling to show how it works
  tokio_stream::iter(vec![
    MyTestStructure {
      some_test_field: "test1".to_string()
    };
    100000
  ])
  .throttle(std::time::Duration::from_secs(1))
}

async fn test_json_array_stream() -> impl IntoResponse {
  StreamBodyAsOptions::new()
    .buffering_ready_items(1000)
    .json_array(source_test_stream())
}

#[tokio::main]
async fn main() {
  // build our application with a route
  let app = Router::new()
    // `GET /` goes to `root`
    .route("/json-array-buffering", get(test_json_array_stream));

  let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

  axum::serve(listener, app).await.unwrap();
}

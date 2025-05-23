use axum::{body::Bytes, response::IntoResponse, routing::*, Router};
use axum_streams::*;
// use std::net::SocketAddr;
use tokio_stream::Stream;
use tokio_stream::StreamExt;

mod mypackage {
  include!("protos/mypackage.rs");
}

fn source_test_stream() -> impl Stream<Item = mypackage::MyMessage> {
  // Simulating a stream with a plain vector and throttling to show how it works
  tokio_stream::iter(vec![
        mypackage::MyMessage {
            content: "test1".to_string()
        };
        5 // Adjust the number of messages as needed
    ])
  .throttle(std::time::Duration::from_secs(1))
  // stream::once(future::ready(mypackage::MyMessage {
  //     content: "test1".to_string(),
  // }))
}

async fn test_protobuf_stream() -> impl IntoResponse {
  StreamBodyAs::protobuf(source_test_stream())
}

async fn echo_handler(input: Bytes) -> impl IntoResponse {
  input
}

#[tokio::main]
async fn main() {
  // build our application with a route
  let app = Router::new()
    // `GET /` goes to `root`
    .route("/", get(echo_handler))
    .route("/", post(echo_handler))
    .route("/protobuf-stream", get(test_protobuf_stream));

  // let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
  let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
  axum::serve(listener, app).await.unwrap();
}

use axum::response::IntoResponse;
use axum::{response::Response, routing::post, Json, Router};
use serde_json::Value;

// curl -X POST http://127.0.0.1:8080/ -H "Content-Type: application/json" -d '{"echo": "Hello, world!"}'

#[tokio::main]
async fn main() {
  // build our application with a route
  let router = Router::new().route("/", post(echo_handler));

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
  println!("listening on {:?}", listener);

  axum::serve(listener, router).await.unwrap();
}

async fn echo_handler(Json(payload): Json<Value>) -> Response {
  // Echoes back the received JSON payload
  // println!("payload: {:?}", payload) ;
  Json(payload).into_response()
}

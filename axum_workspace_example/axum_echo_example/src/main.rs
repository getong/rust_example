use axum::{routing::get, Router};

// curl -X GET http://localhost:3000/ -d "Hello, world!"
async fn echo_handler(payload: String) -> String {
  payload
}

#[tokio::main]
async fn main() {
  let router = Router::new()
    // `GET /` goes to `root`
    .route("/", get(echo_handler));

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  axum::serve(listener, router).await.unwrap();
}

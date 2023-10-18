use axum::{routing::get, Router};

use std::net::SocketAddr;

// curl -X GET http://localhost:3000/ -d "Hello, world!"
async fn echo_handler(payload: String) -> String {
    payload
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(echo_handler));
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

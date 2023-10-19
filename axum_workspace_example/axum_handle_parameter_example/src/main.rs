use axum::{routing::get, Router};
use std::net::SocketAddr;

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
    let app = Router::new().route("/hello", get(query));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

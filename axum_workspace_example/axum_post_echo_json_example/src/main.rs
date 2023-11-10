use axum::response::IntoResponse;
use axum::{response::Response, routing::post, Json, Router};
use serde_json::Value;
use std::net::SocketAddr;

// curl -X POST http://127.0.0.1:8080/ -H "Content-Type: application/json" -d '{"echo": "Hello, world!"}'

#[tokio::main]
async fn main() {
    // build our application with a route
    let app = Router::new().route("/", post(echo_handler));

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn echo_handler(Json(payload): Json<Value>) -> Response {
    // Echoes back the received JSON payload
    // println!("payload: {:?}", payload) ;
    Json(payload).into_response()
}

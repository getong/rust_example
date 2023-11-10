use axum::{
    body::Bytes,
    response::{IntoResponse, Response},
    routing::post,
    Router,
};

use std::net::SocketAddr;

// Import the generated Rust code for the Protobuf definitions.
// mod echo_message {
//     include!("echo.rs");
// }

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", post(echo_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn echo_handler(bytes: Bytes) -> Response {
    // println!("bytes: {:?}\n", bytes);
    bytes.into_response()
}

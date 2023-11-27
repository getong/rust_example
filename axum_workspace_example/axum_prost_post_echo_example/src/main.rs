use axum::{
    body::Bytes,
    response::{IntoResponse, Response},
    routing::post,
    Router,
};

// Import the generated Rust code for the Protobuf definitions.
// mod echo_message {
//     include!("echo.rs");
// }

#[tokio::main]
async fn main() {
    let router = Router::new().route("/", post(echo_handler));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, router).await.unwrap();
}

async fn echo_handler(bytes: Bytes) -> Response {
    // println!("bytes: {:?}\n", bytes);
    bytes.into_response()
}

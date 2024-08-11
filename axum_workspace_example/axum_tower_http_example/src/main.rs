use axum::{response::Json, routing::get, Router};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() {
  // Create a CORS layer that allows any origin
  let cors = CorsLayer::new()
    .allow_origin(Any) // Allow requests from any origin
    // .allow_methods(vec!["GET", "POST"])  // Allow GET and POST methods
    .allow_headers(Any); // Allow any header

  // Create a router
  let app = Router::new().route("/", get(handler)).layer(cors); // Apply the CORS layer to the app

  // Define the socket address
  let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
  println!("Listening on {}", addr);
  let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

  // Run the app
  axum::serve(listener, app.into_make_service())
    .await
    .unwrap();
}

// Basic handler that returns a JSON response
async fn handler() -> Json<&'static str> {
  Json("Hello, World!")
}

use axum::{routing::get_service, Router};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
  // Serve the "assets" directory
  let service = get_service(ServeDir::new("assets")).handle_error(|error| async move {
    (
      axum::http::StatusCode::INTERNAL_SERVER_ERROR,
      format!("Unhandled internal error: {}", error),
    )
  });

  // Build the Router with a route to serve static files directly
  let router = Router::new().fallback_service(service);

  // Bind to a TCP listener
  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  println!("Listening on http://localhost:3000/");

  // Start the server
  axum::serve(listener, router).await.unwrap();
}

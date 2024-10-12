use axum::{routing::get_service, Router};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
  let service = get_service(ServeDir::new("assets")).handle_error(|error| async move {
    (
      axum::http::StatusCode::INTERNAL_SERVER_ERROR,
      format!("Unhandled internal error: {}", error),
    )
  });

  // Wrap the service in a Router
  let assets_router = Router::new().route_service("/", service);

  // Build our application with a route to serve static files
  let router = Router::new().nest("/", assets_router);

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  println!("listen to http://localhost:3000/");

  axum::serve(listener, router).await.unwrap();
}

use axum::{
  extract::Query,
  response::{Html, IntoResponse},
  routing::{get, get_service},
  Router,
};
use axum_extra::response::JavaScript;
use serde::Deserialize;
use tower::ServiceBuilder;
use tower_http::{services::ServeDir, trace::TraceLayer};

#[tokio::main]
async fn main() {
  // Create the main app router
  let app = Router::new()
    // Serve static JavaScript files under /js path
    .nest("/js", js_service())
    // Route to serve inline JavaScript code
    .route("/inline-js", get(inline_js_handler))
    // Route to handle query parameters and respond with HTML
    .route("/greet", get(query_greeting_handler))
    // Add logging
    .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

  // Define the socket address and start the server
  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
  println!("Listening on http://localhost:3000");
  axum::serve(listener, app).await.unwrap();
}

// Inline JavaScript handler
async fn inline_js_handler() -> impl IntoResponse {
  JavaScript("console.log('Hello from Axum!');")
}

// Route to serve static JavaScript files
// Static file service for JavaScript files
fn js_service() -> Router {
  Router::new().route(
    "/",
    get_service(ServeDir::new("javascript_files")).handle_error(|error| async move {
      (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("Unhandled internal error: {}", error),
      )
    }),
  )
}

// Handler to respond to query parameters
#[derive(Deserialize)]
struct Params {
  name: String,
}

async fn query_greeting_handler(Query(params): Query<Params>) -> impl IntoResponse {
  Html(format!("<h1>Hello, {}!</h1>", params.name))
}

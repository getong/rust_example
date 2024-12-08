use axum::{
  extract::Path,
  http::StatusCode,
  response::IntoResponse,
  routing::{get, get_service, post},
  Json, Router,
};
use http::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::{
  cors::{Any, CorsLayer},
  services::ServeFile,
};

#[derive(Deserialize)]
struct CreateUser {
  username: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, Hash, PartialEq)]
struct User {
  id: u64,
  username: String,
}

#[tokio::main]
async fn main() {
  tracing_subscriber::fmt::init();

  let cors = CorsLayer::new()
    // allow `GET` and `POST` when accessing the resource
    .allow_methods([Method::GET, Method::POST])
    // allow requests from any origin
    .allow_origin(Any);

  let router = Router::new()
    .route("/", get(root))
    .route("/user", post(create_user))
    .route("/hello/:name", get(json_hello))
    .route("/static", get_service(ServeFile::new("static/hello.html")))
    .layer(cors);

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  axum::serve(listener, router).await.unwrap();
}

async fn root() -> &'static str {
  "Hello, World!"
}

async fn create_user(Json(payload): Json<CreateUser>) -> impl IntoResponse {
  let user = User {
    id: 1337,
    username: payload.username,
  };

  (StatusCode::CREATED, Json(user))
}

async fn json_hello(Path(name): Path<String>) -> impl IntoResponse {
  let greeting = name.as_str();
  let hello = String::from("Hello ");

  (StatusCode::OK, Json(json!({ "message": hello + greeting })))
}

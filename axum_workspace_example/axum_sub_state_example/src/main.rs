use axum::{
  Json, Router,
  extract::{FromRef, State},
  http::StatusCode,
  response::IntoResponse,
  routing::{get, post},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct CreateUser {
  username: String,
}

#[derive(Serialize)]
struct User {
  id: u64,
  username: String,
}

// the application state
#[derive(Clone, Debug)]
struct AppState {
  // that holds some api specific state
  api_state: ApiState,
}

// the api specific state
#[derive(Clone, Debug)]
struct ApiState {}

// support converting an `AppState` in an `ApiState`
impl FromRef<AppState> for ApiState {
  fn from_ref(app_state: &AppState) -> ApiState {
    app_state.api_state.clone()
  }
}

async fn create_user(Json(payload): Json<CreateUser>) -> impl IntoResponse {
  let user = User {
    id: 1337,
    username: payload.username,
  };

  (StatusCode::CREATED, Json(user))
}

async fn api_users(
  // access the api specific state
  State(api_state): State<ApiState>,
  Json(payload): Json<CreateUser>,
) -> impl IntoResponse {
  println!("api_state is {:?}", api_state);
  let user = User {
    id: 1337,
    username: payload.username,
  };

  (StatusCode::CREATED, Json(user))
}

async fn handler(
  // we can still access to top level state
  State(state): State<AppState>,
) -> &'static str {
  println!("app state is {:?}", state);
  "Hello, World!"
}

#[tokio::main]
async fn main() {
  let state = AppState {
    api_state: ApiState {},
  };

  let router = Router::new()
    .route("/", get(handler))
    .route("/api/users", post(api_users))
    .route("/create_user", post(create_user))
    .with_state(state);

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  axum::serve(listener, router).await.unwrap();
}

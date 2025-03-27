use axum::{
  Json, Router,
  extract::{FromRef, State},
  http::StatusCode,
  response::IntoResponse,
  routing::{get, post},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
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

// curl -X POST http://localhost:3000/create_user \
// -H "Content-Type: application/json" \
// -d '{"username": "abc"}'

async fn create_user(
  State(app_state): State<AppState>,
  Json(payload): Json<CreateUser>,
) -> impl IntoResponse {
  println!("app_state is {:?}", app_state);
  let user = User {
    id: 1337,
    username: payload.username,
  };

  (StatusCode::CREATED, Json(user))
}

// curl -X POST http://localhost:3000/api/create_user \
// -H "Content-Type: application/json" \
// -d '{"username": "abc", "password": "123"}'

async fn api_users(
  // access the api specific state
  State(api_state): State<ApiState>,
  Json(payload): Json<CreateUser>,
) -> impl IntoResponse {
  println!("api_state is {:?}", api_state);
  println!("payload is {:?}", payload);
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
    .route("/create_user", post(create_user))
    .nest("/api", Router::new().route("/create_user", post(api_users)))
    .with_state(state);

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  axum::serve(listener, router).await.unwrap();
}

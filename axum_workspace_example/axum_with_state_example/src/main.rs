use std::{net::SocketAddr, sync::Arc};

use axum::{Json, Router, extract::State, routing::get};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Clone)]
struct AppState {
  counter: Arc<RwLock<i32>>,
}

#[derive(Serialize, Deserialize)]
struct CounterResponse {
  count: i32,
}

#[tokio::main]
async fn main() {
  let state = Arc::new(AppState {
    counter: Arc::new(RwLock::new(0)),
  });

  let app = Router::new()
    .route("/", get(get_counter).post(increment_counter))
    .with_state(state);

  let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
  let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
  axum::serve(listener, app).await.unwrap();
}

async fn get_counter(State(state): State<Arc<AppState>>) -> Json<CounterResponse> {
  let counter = state.counter.read().await;
  Json(CounterResponse { count: *counter })
}

async fn increment_counter(
  State(state): State<Arc<AppState>>,
  _: Json<serde_json::Value>, // Ignore the body of the POST request
) -> Json<CounterResponse> {
  let mut counter = state.counter.write().await;
  *counter += 1;
  Json(CounterResponse { count: *counter })
}

use std::net::SocketAddr;

use axum::{response::IntoResponse, routing::get, Router};
use serde::{Deserialize, Serialize};
use time::Duration;
use tower_sessions::{cookie::Key, Expiry, MemoryStore, Session, SessionManagerLayer};

const COUNTER_KEY: &str = "counter";

#[derive(Default, Deserialize, Serialize)]
struct Counter(usize);

async fn handler(session: Session) -> impl IntoResponse {
  println!("session: {:?}", session);
  let counter: Counter = session.get(COUNTER_KEY).await.unwrap().unwrap_or_default();
  session.insert(COUNTER_KEY, counter.0 + 1).await.unwrap();
  format!("Current count: {}", counter.0)
}

#[tokio::main]
async fn main() {
  let key = Key::generate();

  let session_store = MemoryStore::default();
  let session_layer = SessionManagerLayer::new(session_store)
    .with_signed(key)
    .with_secure(false)
    .with_expiry(Expiry::OnInactivity(Duration::seconds(10)));

  let app = Router::new().route("/", get(handler)).layer(session_layer);

  let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
  let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
  axum::serve(listener, app.into_make_service())
    .await
    .unwrap();
}

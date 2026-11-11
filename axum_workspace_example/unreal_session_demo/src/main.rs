mod auth;
mod error;
mod sessions;

use std::{collections::HashMap, env, error::Error, sync::Arc};

use axum::{
  Router, middleware,
  routing::{get, post, put},
};
use jsonwebtoken::{DecodingKey, EncodingKey};
use tokio::{net::TcpListener, sync::RwLock};
use uuid::Uuid;

use sessions::Session;

#[derive(Clone)]
struct AppState {
  sessions: Arc<RwLock<HashMap<Uuid, Session>>>,
  encoding_key: EncodingKey,
  decoding_key: DecodingKey,
  game_server_addr: Arc<str>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let secret =
    env::var("JWT_SECRET").map_err(|_| "set JWT_SECRET to a random secret of at least 32 bytes")?;
  if secret.len() < 32 {
    return Err("JWT_SECRET must contain at least 32 bytes".into());
  }
  let state = AppState {
    sessions: Arc::default(),
    encoding_key: EncodingKey::from_secret(secret.as_bytes()),
    decoding_key: DecodingKey::from_secret(secret.as_bytes()),
    game_server_addr: env::var("GAME_SERVER_ADDR")
      .unwrap_or_else(|_| "127.0.0.1:7777".to_owned())
      .into(),
  };
  let protected = Router::new()
    .route("/sessions", get(sessions::find).post(sessions::create))
    .route(
      "/sessions/{id}",
      get(sessions::get).delete(sessions::destroy),
    )
    .route("/sessions/{id}/settings", put(sessions::update))
    .route("/sessions/{id}/join", post(sessions::join))
    .route("/sessions/{id}/leave", post(sessions::leave))
    .route("/sessions/{id}/start", post(sessions::start))
    .route("/sessions/{id}/end", post(sessions::end))
    .route_layer(middleware::from_fn_with_state(
      state.clone(),
      auth::authenticate,
    ));
  let app = Router::new()
    .route("/health", get(|| async { "ok" }))
    .route("/auth/login", post(auth::login))
    .route("/game/connect", post(sessions::connect))
    .merge(protected)
    .with_state(state);
  let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
  let listener = TcpListener::bind(&bind_addr).await?;
  println!("Session service: http://{}", listener.local_addr()?);
  println!("Demo accounts: alice / bob / charlie; password: demo-password");
  println!("Game transport is NOT running; /game/connect only simulates admission.");
  axum::serve(listener, app)
    .with_graceful_shutdown(async {
      if let Err(error) = tokio::signal::ctrl_c().await {
        eprintln!("failed to listen for shutdown: {error}");
      }
    })
    .await?;
  Ok(())
}

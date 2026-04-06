mod auth;
mod config;
mod error;
mod handlers;
mod models;
mod websocket;

use std::{env, path::PathBuf};

use axum::{
  Router,
  routing::{get, post},
};
use axum_server::tls_rustls::RustlsConfig;
use dotenvy::dotenv;
use supabase_auth::models::AuthClient;
use supabase_rs::SupabaseClient;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

use crate::{config::AppConfig, models::AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  tracing_subscriber::fmt()
    .with_env_filter(
      tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "supabase_rs_signup_signin_example=debug,tower_http=info".into()),
    )
    .init();

  dotenv().ok();

  let config = AppConfig::from_env()?;
  let supabase_url = env::var("SUPABASE_URL")?;
  let supabase_key = env::var("SUPABASE_KEY")?;
  let client =
    SupabaseClient::new(supabase_url.clone(), supabase_key.clone())?.schema(&config.schema);
  let supabase_auth_api_key = env::var("SUPABASE_API_KEY").unwrap_or_else(|_| supabase_key.clone());
  let supabase_auth_client = env::var("SUPABASE_JWT_SECRET")
    .ok()
    .map(|jwt_secret| AuthClient::new(supabase_url.clone(), supabase_auth_api_key, jwt_secret));
  let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

  let state = AppState {
    client,
    supabase_auth_client,
    config,
  };
  let app = Router::new()
    .route("/", get(handlers::index))
    .route("/health", get(handlers::health))
    .route("/auth/signup", post(handlers::signup))
    .route("/auth/signin", post(handlers::signin))
    .route(
      "/auth/supabase/signup",
      post(handlers::supabase_auth_signup),
    )
    .route(
      "/auth/supabase/signin",
      post(handlers::supabase_auth_signin),
    )
    .route("/ws", get(websocket::ws_handler))
    .route("/ws/demo", get(websocket::ws_demo_page))
    .nest_service("/assets", ServeDir::new(assets_dir))
    .with_state(state.clone());

  match (&state.config.tls_cert_path, &state.config.tls_key_path) {
    (Some(cert_path), Some(key_path)) => {
      let tls_config = RustlsConfig::from_pem_file(cert_path, key_path).await?;
      tracing::info!(
        "listening on https://{} with wss://{}/ws schema={} table={} supabase_auth={}",
        state.config.bind_addr,
        state.config.bind_addr,
        state.config.schema,
        state.config.table,
        if state.supabase_auth_client.is_some() {
          "enabled"
        } else {
          "disabled"
        }
      );

      axum_server::bind_rustls(state.config.bind_addr, tls_config)
        .serve(app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .await?;
    }
    (None, None) => {
      let listener = TcpListener::bind(state.config.bind_addr).await?;
      tracing::info!(
        "listening on http://{} with ws://{}/ws schema={} table={} supabase_auth={}",
        state.config.bind_addr,
        state.config.bind_addr,
        state.config.schema,
        state.config.table,
        if state.supabase_auth_client.is_some() {
          "enabled"
        } else {
          "disabled"
        }
      );

      axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
      )
      .await?;
    }
    _ => {
      return Err("TLS_CERT_PATH and TLS_KEY_PATH must either both be set or both be unset".into());
    }
  }

  Ok(())
}

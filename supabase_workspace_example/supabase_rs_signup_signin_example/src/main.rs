mod auth;
mod config;
mod error;
mod handlers;
mod models;

use std::env;

use axum::{
  Router,
  routing::{get, post},
};
use dotenvy::dotenv;
use supabase_auth::models::AuthClient;
use supabase_rs::SupabaseClient;
use tokio::net::TcpListener;

use crate::{config::AppConfig, models::AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    .with_state(state.clone());

  let listener = TcpListener::bind(state.config.bind_addr).await?;
  println!(
    "listening on http://{} using schema={} table={} supabase_auth={}",
    state.config.bind_addr,
    state.config.schema,
    state.config.table,
    if state.supabase_auth_client.is_some() {
      "enabled"
    } else {
      "disabled"
    }
  );

  axum::serve(listener, app).await?;
  Ok(())
}

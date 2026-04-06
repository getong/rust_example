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
use supabase_rs::SupabaseClient;
use tokio::net::TcpListener;

use crate::{config::AppConfig, models::AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  dotenv().ok();

  let config = AppConfig::from_env()?;
  let client = SupabaseClient::new(env::var("SUPABASE_URL")?, env::var("SUPABASE_KEY")?)?
    .schema(&config.schema);

  let state = AppState { client, config };
  let app = Router::new()
    .route("/", get(handlers::index))
    .route("/health", get(handlers::health))
    .route("/auth/signup", post(handlers::signup))
    .route("/auth/signin", post(handlers::signin))
    .with_state(state.clone());

  let listener = TcpListener::bind(state.config.bind_addr).await?;
  println!(
    "listening on http://{} using schema={} table={}",
    state.config.bind_addr, state.config.schema, state.config.table
  );

  axum::serve(listener, app).await?;
  Ok(())
}

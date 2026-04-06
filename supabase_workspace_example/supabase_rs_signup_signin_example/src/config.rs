use std::{env, net::SocketAddr};

use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct AppConfig {
  pub bind_addr: SocketAddr,
  pub table: String,
  pub schema: String,
  pub token_secret: Vec<u8>,
  pub token_ttl_seconds: u64,
  pub password_pepper: String,
}

impl AppConfig {
  pub fn from_env() -> AppResult<Self> {
    let bind_addr = read_env_or("APP_ADDR", "127.0.0.1:3000")
      .parse()
      .map_err(|err| AppError::internal(format!("invalid APP_ADDR: {err}")))?;

    let token_secret = env::var("AUTH_TOKEN_SECRET")
      .map_err(|_| AppError::internal("AUTH_TOKEN_SECRET must be set".to_owned()))?
      .into_bytes();

    if token_secret.len() < 32 {
      return Err(AppError::internal(
        "AUTH_TOKEN_SECRET must be at least 32 bytes".to_owned(),
      ));
    }

    Ok(Self {
      bind_addr,
      table: read_env_or("SUPABASE_AUTH_TABLE", "app_users"),
      schema: read_env_or("SUPABASE_SCHEMA", "public"),
      token_secret,
      token_ttl_seconds: read_env_u64_or("AUTH_TOKEN_TTL_SECONDS", 60 * 60 * 24),
      password_pepper: env::var("AUTH_PASSWORD_PEPPER").unwrap_or_default(),
    })
  }
}

pub fn read_env_or(key: &str, default: &str) -> String {
  env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn read_env_u64_or(key: &str, default: u64) -> u64 {
  env::var(key)
    .ok()
    .and_then(|value| value.parse::<u64>().ok())
    .unwrap_or(default)
}

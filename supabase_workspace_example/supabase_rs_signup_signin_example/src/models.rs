use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use supabase_rs::SupabaseClient;

use crate::config::AppConfig;

#[derive(Clone)]
pub struct AppState {
  pub client: SupabaseClient,
  pub config: AppConfig,
}

#[derive(Debug, Deserialize)]
pub struct AuthRequest {
  pub email: String,
  pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
  pub user_id: String,
  pub email: String,
  pub access_token: String,
  pub token_type: &'static str,
  pub expires_in: u64,
}

#[derive(Debug, Serialize)]
pub struct ApiMessage {
  pub message: String,
}

#[derive(Debug, Serialize)]
pub struct IndexResponse {
  pub service: &'static str,
  pub endpoints: [&'static str; 3],
  pub required_env: [&'static str; 3],
  pub optional_env: [&'static str; 4],
  pub recommended_sql: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct StoredUserRow {
  #[serde(deserialize_with = "deserialize_stringish")]
  pub id: String,
  pub email: String,
  pub password_hash: String,
}

#[derive(Debug, Serialize)]
pub struct SessionClaims<'a> {
  pub sub: &'a str,
  pub email: &'a str,
  pub iat: u64,
  pub exp: u64,
}

fn deserialize_stringish<'de, D>(deserializer: D) -> Result<String, D::Error>
where
  D: Deserializer<'de>,
{
  let value = Value::deserialize(deserializer)?;
  Ok(match value {
    Value::String(inner) => inner,
    other => other.to_string(),
  })
}

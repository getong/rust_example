use axum::{Json, extract::State, http::StatusCode};
use serde_json::json;

use crate::{
  auth::{hash_password, issue_token, validate_credentials, verify_password},
  error::{AppError, AppResult},
  models::{ApiMessage, AppState, AuthResponse, IndexResponse, StoredUserRow},
};

pub async fn index() -> Json<IndexResponse> {
  Json(IndexResponse {
    service: "supabase_rs signup/signin example",
    endpoints: ["GET /health", "POST /auth/signup", "POST /auth/signin"],
    required_env: ["SUPABASE_URL", "SUPABASE_KEY", "AUTH_TOKEN_SECRET"],
    optional_env: [
      "APP_ADDR",
      "SUPABASE_SCHEMA",
      "SUPABASE_AUTH_TABLE",
      "AUTH_PASSWORD_PEPPER",
    ],
    recommended_sql:
      "create extension if not exists pgcrypto;\ncreate table public.app_users (\n  id uuid \
       primary key default gen_random_uuid(),\n  email text not null unique,\n  password_hash text \
       not null,\n  created_at timestamptz not null default now()\n);",
  })
}

pub async fn health() -> Json<ApiMessage> {
  Json(ApiMessage {
    message: "ok".to_owned(),
  })
}

pub async fn signup(
  State(state): State<AppState>,
  Json(payload): Json<crate::models::AuthRequest>,
) -> AppResult<(StatusCode, Json<AuthResponse>)> {
  let credentials = validate_credentials(payload)?;

  if find_user_by_email(&state, &credentials.email)
    .await?
    .is_some()
  {
    return Err(AppError::conflict("email already exists"));
  }

  let password_hash = hash_password(&credentials.password, &state.config.password_pepper)?;

  let inserted_id = state
    .client
    .insert(
      &state.config.table,
      json!({
        "email": credentials.email,
        "password_hash": password_hash,
      }),
    )
    .await
    .map(normalize_supabase_error)
    .map_err(map_insert_error)?;

  let access_token = issue_token(
    &inserted_id,
    &credentials.email,
    state.config.token_ttl_seconds,
    &state.config.token_secret,
  )?;

  Ok((
    StatusCode::CREATED,
    Json(AuthResponse {
      user_id: inserted_id,
      email: credentials.email,
      access_token,
      token_type: "Bearer",
      expires_in: state.config.token_ttl_seconds,
    }),
  ))
}

pub async fn signin(
  State(state): State<AppState>,
  Json(payload): Json<crate::models::AuthRequest>,
) -> AppResult<Json<AuthResponse>> {
  let credentials = validate_credentials(payload)?;
  let user = find_user_by_email(&state, &credentials.email)
    .await?
    .ok_or_else(|| AppError::unauthorized("invalid email or password"))?;

  if !verify_password(
    &credentials.password,
    &user.password_hash,
    &state.config.password_pepper,
  )? {
    return Err(AppError::unauthorized("invalid email or password"));
  }

  let access_token = issue_token(
    &user.id,
    &user.email,
    state.config.token_ttl_seconds,
    &state.config.token_secret,
  )?;

  Ok(Json(AuthResponse {
    user_id: user.id,
    email: user.email,
    access_token,
    token_type: "Bearer",
    expires_in: state.config.token_ttl_seconds,
  }))
}

async fn find_user_by_email(state: &AppState, email: &str) -> AppResult<Option<StoredUserRow>> {
  let row = state
    .client
    .from(&state.config.table)
    .eq("email", email)
    .first()
    .await
    .map_err(|err| AppError::internal(format!("supabase select failed: {err}")))?;

  row
    .map(|value| {
      serde_json::from_value(value)
        .map_err(|err| AppError::internal(format!("failed to decode user row: {err}")))
    })
    .transpose()
}

fn normalize_supabase_error(err: String) -> String {
  err.trim_matches('"').to_owned()
}

fn map_insert_error(err: String) -> AppError {
  if err.contains("409") {
    AppError::conflict("email already exists")
  } else {
    AppError::internal(format!("supabase insert failed: {err}"))
  }
}

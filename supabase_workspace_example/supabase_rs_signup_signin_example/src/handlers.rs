use axum::{Json, extract::State, http::StatusCode};
use serde_json::json;
use supabase_auth::{
  error::Error as SupabaseAuthError,
  models::{EmailSignUpResult, Session, SignUpWithPasswordOptions},
};

use crate::{
  auth::{hash_password, issue_token, validate_credentials, verify_password},
  error::{AppError, AppResult},
  models::{
    ApiMessage, AppState, AuthResponse, IndexResponse, StoredUserRow, SupabaseAuthResponse,
  },
};

pub async fn index() -> Json<IndexResponse> {
  Json(IndexResponse {
    service: "supabase_rs and supabase_auth signup/signin example",
    endpoints: [
      "GET /health",
      "POST /auth/signup",
      "POST /auth/signin",
      "POST /auth/supabase/signup",
      "POST /auth/supabase/signin",
    ],
    required_env: ["SUPABASE_URL", "SUPABASE_KEY", "AUTH_TOKEN_SECRET"],
    optional_env: [
      "APP_ADDR",
      "SUPABASE_SCHEMA",
      "SUPABASE_AUTH_TABLE",
      "AUTH_PASSWORD_PEPPER",
      "SUPABASE_API_KEY",
      "SUPABASE_JWT_SECRET",
      "SUPABASE_AUTH_EMAIL_REDIRECT_TO",
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

pub async fn supabase_auth_signup(
  State(state): State<AppState>,
  Json(payload): Json<crate::models::AuthRequest>,
) -> AppResult<(StatusCode, Json<SupabaseAuthResponse>)> {
  let credentials = validate_credentials(payload)?;
  let auth_client = state.supabase_auth_client.as_ref().ok_or_else(|| {
    AppError::service_unavailable(
      "supabase_auth is not configured; set SUPABASE_JWT_SECRET to enable these handlers",
    )
  })?;

  let options = state
    .config
    .supabase_auth_email_redirect_to
    .as_ref()
    .map(|redirect_to| SignUpWithPasswordOptions {
      email_redirect_to: Some(redirect_to.clone()),
      ..Default::default()
    });

  let result = auth_client
    .sign_up_with_email_and_password(&credentials.email, &credentials.password, options)
    .await
    .map_err(map_supabase_auth_error)?;

  let response = match result {
    EmailSignUpResult::SessionResult(session) => {
      response_from_supabase_session(session, "signed up with supabase_auth".to_owned(), None)
    }
    EmailSignUpResult::ConfirmationResult(confirmation) => SupabaseAuthResponse {
      provider: "supabase_auth",
      user_id: confirmation.id.to_string(),
      email: confirmation.email,
      access_token: None,
      refresh_token: None,
      token_type: None,
      expires_in: None,
      expires_at: None,
      message: "confirmation email sent by supabase_auth".to_owned(),
      confirmation_sent_at: Some(confirmation.confirmation_sent_at),
    },
  };

  Ok((StatusCode::CREATED, Json(response)))
}

pub async fn supabase_auth_signin(
  State(state): State<AppState>,
  Json(payload): Json<crate::models::AuthRequest>,
) -> AppResult<Json<SupabaseAuthResponse>> {
  let credentials = validate_credentials(payload)?;
  let auth_client = state.supabase_auth_client.as_ref().ok_or_else(|| {
    AppError::service_unavailable(
      "supabase_auth is not configured; set SUPABASE_JWT_SECRET to enable these handlers",
    )
  })?;

  let session = auth_client
    .login_with_email(&credentials.email, &credentials.password)
    .await
    .map_err(map_supabase_auth_error)?;

  Ok(Json(response_from_supabase_session(
    session,
    "signed in with supabase_auth".to_owned(),
    None,
  )))
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

fn response_from_supabase_session(
  session: Session,
  message: String,
  confirmation_sent_at: Option<String>,
) -> SupabaseAuthResponse {
  SupabaseAuthResponse {
    provider: "supabase_auth",
    user_id: session.user.id.to_string(),
    email: Some(session.user.email),
    access_token: Some(session.access_token),
    refresh_token: Some(session.refresh_token),
    token_type: Some(session.token_type),
    expires_in: Some(session.expires_in),
    expires_at: Some(session.expires_at),
    message,
    confirmation_sent_at,
  }
}

fn map_supabase_auth_error(err: SupabaseAuthError) -> AppError {
  match err {
    SupabaseAuthError::AlreadySignedUp => AppError::conflict("user already exists"),
    SupabaseAuthError::WrongCredentials | SupabaseAuthError::UserNotFound => {
      AppError::unauthorized("invalid email or password")
    }
    SupabaseAuthError::AuthError { status, message } => {
      let status_code = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
      AppError {
        status: status_code,
        message,
      }
    }
    other => AppError::internal(format!("supabase_auth request failed: {other}")),
  }
}

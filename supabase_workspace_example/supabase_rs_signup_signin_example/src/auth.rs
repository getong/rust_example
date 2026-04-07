use std::time::{SystemTime, UNIX_EPOCH};

use argon2::{
  Argon2,
  password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use axum::http::StatusCode;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use supabase_auth::error::Error as SupabaseAuthError;

use crate::{
  error::{AppError, AppResult},
  models::{AuthRequest, SessionClaims},
};

type HmacSha256 = Hmac<Sha256>;

pub fn validate_credentials(payload: AuthRequest) -> AppResult<AuthRequest> {
  let email = payload.email.trim().to_lowercase();
  let password = payload.password;

  if email.is_empty() || !email.contains('@') {
    return Err(AppError::bad_request("email must be a valid address"));
  }

  if password.len() < 8 {
    return Err(AppError::bad_request(
      "password must be at least 8 characters",
    ));
  }

  Ok(AuthRequest { email, password })
}

pub fn hash_password(password: &str, pepper: &str) -> AppResult<String> {
  let hashed = Argon2::default()
    .hash_password(password_with_pepper(password, pepper).as_bytes())
    .map_err(|err| AppError::internal(format!("password hash failed: {err}")))?
    .to_string();

  Ok(hashed)
}

pub fn verify_password(password: &str, password_hash: &str, pepper: &str) -> AppResult<bool> {
  let parsed_hash = PasswordHash::new(password_hash)
    .map_err(|err| AppError::internal(format!("stored password hash is invalid: {err}")))?;

  Ok(
    Argon2::default()
      .verify_password(
        password_with_pepper(password, pepper).as_bytes(),
        &parsed_hash,
      )
      .is_ok(),
  )
}

pub fn issue_token(
  user_id: &str,
  email: &str,
  ttl_seconds: u64,
  secret: &[u8],
) -> AppResult<String> {
  let issued_at = now_unix_seconds()?;
  let claims = SessionClaims {
    sub: user_id,
    email,
    iat: issued_at,
    exp: issued_at.saturating_add(ttl_seconds),
  };

  let payload = serde_json::to_vec(&claims)
    .map_err(|err| AppError::internal(format!("failed to serialize token payload: {err}")))?;
  let payload = URL_SAFE_NO_PAD.encode(payload);

  let mut mac = HmacSha256::new_from_slice(secret)
    .map_err(|err| AppError::internal(format!("invalid token secret: {err}")))?;
  mac.update(payload.as_bytes());
  let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());

  Ok(format!("{payload}.{signature}"))
}

pub fn map_supabase_auth_error(err: SupabaseAuthError) -> AppError {
  match err {
    SupabaseAuthError::AlreadySignedUp => AppError::conflict("user already exists"),
    SupabaseAuthError::WrongCredentials | SupabaseAuthError::UserNotFound => {
      AppError::unauthorized("invalid email or password")
    }
    SupabaseAuthError::WrongToken | SupabaseAuthError::NotAuthenticated => {
      AppError::unauthorized("invalid or expired supabase access token")
    }
    SupabaseAuthError::AuthError { status, message } => {
      let status_code = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
      let normalized_message = message.to_ascii_lowercase();

      if normalized_message.contains("invalid login credentials") {
        return AppError::unauthorized(
          "invalid email or password, or the Supabase Auth user does not exist yet",
        );
      }

      if normalized_message.contains("email not confirmed") {
        return AppError::unauthorized("email is not confirmed for this Supabase Auth user");
      }

      AppError {
        status: status_code,
        message,
      }
    }
    other => AppError::internal(format!("supabase_auth request failed: {other}")),
  }
}

fn password_with_pepper(password: &str, pepper: &str) -> String {
  if pepper.is_empty() {
    return password.to_owned();
  }

  format!("{password}:{pepper}")
}

fn now_unix_seconds() -> AppResult<u64> {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|duration| duration.as_secs())
    .map_err(|err| AppError::internal(format!("system clock error: {err}")))
}

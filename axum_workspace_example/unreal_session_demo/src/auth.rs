use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
  Extension, Json,
  extract::{Request, State},
  http::header::AUTHORIZATION,
  middleware::Next,
  response::Response,
};
use jsonwebtoken::{Algorithm, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, error::ApiError};

pub(crate) const API_AUDIENCE: &str = "session-api";
pub(crate) const GAME_AUDIENCE: &str = "game-server";
const ISSUER: &str = "unreal-session-demo";

#[derive(Clone)]
pub(crate) struct Player(pub String);

#[derive(Serialize, Deserialize)]
pub(crate) struct Claims {
  pub sub: String,
  aud: String,
  iss: String,
  iat: u64,
  exp: u64,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub session_id: Option<Uuid>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub membership_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoginRequest {
  username: String,
  password: String,
}

#[derive(Serialize)]
pub(crate) struct LoginResponse {
  access_token: String,
  token_type: &'static str,
  expires_in: u64,
}

pub(crate) fn issue(
  state: &AppState,
  player: &str,
  audience: &str,
  lifetime: u64,
  membership: Option<(Uuid, Uuid)>,
) -> Result<String, ApiError> {
  let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_err(|_| ApiError::internal())?
    .as_secs();
  let claims = Claims {
    sub: player.to_owned(),
    aud: audience.to_owned(),
    iss: ISSUER.to_owned(),
    iat: now,
    exp: now + lifetime,
    session_id: membership.map(|value| value.0),
    membership_id: membership.map(|value| value.1),
  };
  encode(&Header::new(Algorithm::HS256), &claims, &state.encoding_key)
    .map_err(|_| ApiError::internal())
}

pub(crate) fn verify(state: &AppState, token: &str, audience: &str) -> Result<Claims, ApiError> {
  let mut validation = Validation::new(Algorithm::HS256);
  validation.set_audience(&[audience]);
  validation.set_issuer(&[ISSUER]);
  validation.set_required_spec_claims(&["sub", "aud", "iss", "iat", "exp"]);
  validation.leeway = 0;
  decode::<Claims>(token, &state.decoding_key, &validation)
    .map(|data| data.claims)
    .map_err(|_| ApiError::unauthorized())
}

pub(crate) async fn login(
  State(state): State<AppState>,
  Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
  if !matches!(request.username.as_str(), "alice" | "bob" | "charlie")
    || request.password != "demo-password"
  {
    return Err(ApiError::unauthorized());
  }
  Ok(Json(LoginResponse {
    access_token: issue(&state, &request.username, API_AUDIENCE, 3600, None)?,
    token_type: "Bearer",
    expires_in: 3600,
  }))
}

pub(crate) async fn authenticate(
  State(state): State<AppState>,
  mut request: Request,
  next: Next,
) -> Result<Response, ApiError> {
  let token = request
    .headers()
    .get(AUTHORIZATION)
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.strip_prefix("Bearer "))
    .ok_or_else(ApiError::unauthorized)?;
  let claims = verify(&state, token, API_AUDIENCE)?;
  request.extensions_mut().insert(Player(claims.sub));
  Ok(next.run(request).await)
}

pub(crate) type AuthenticatedPlayer = Extension<Player>;

use std::{net::SocketAddr, sync::Arc};

use aws_lc_rs::aead::NONCE_LEN;
use axum::{
  Json,
  extract::{ConnectInfo, Path, State, rejection::JsonRejection, ws::WebSocketUpgrade},
  response::IntoResponse,
};
use zeroize::Zeroize;

use crate::{
  AppState, chat,
  db::{find_user_credentials, insert_registered_user},
  error::ApiError,
  models::{LoginResponse, PublicKeyResponse, RegisterRequest, RegisterResponse},
  password::{hash_password_for_storage, verify_password_for_login},
  service::decrypt_auth_request,
};

pub(crate) fn map_register_request_rejection(rejection: JsonRejection) -> ApiError {
  match rejection {
    JsonRejection::JsonDataError(_) | JsonRejection::JsonSyntaxError(_) => {
      ApiError::bad_request("invalid_request_json", "request body must be valid JSON")
    }
    JsonRejection::MissingJsonContentType(_) => ApiError::unsupported_media_type(
      "missing_json_content_type",
      "Content-Type must be application/json",
    ),
    JsonRejection::BytesRejection(_) => {
      ApiError::bad_request("invalid_request_body", "request body could not be read")
    }
    _ => ApiError::payload_too_large(
      "request_body_too_large",
      "request body exceeds the allowed size",
    ),
  }
}

pub(crate) async fn get_public_key_handler(
  State(state): State<AppState>,
) -> Json<PublicKeyResponse> {
  Json(PublicKeyResponse {
    transport: "RSA-OAEP-256 + AES-256-GCM",
    key_encryption_algorithm: "RSA-OAEP-256",
    content_encryption_algorithm: "AES-256-GCM",
    key_format: "X.509 SubjectPublicKeyInfo PEM",
    public_key_pem: state.crypto.public_key_pem.clone(),
    public_key_der_base64: state.crypto.public_key_der_base64.clone(),
    sha256_hash: state.crypto.public_key_sha256.clone(),
    wrapped_key_bytes: state.crypto.wrapped_key_bytes,
    nonce_bytes: NONCE_LEN,
    max_wrapped_key_plaintext_bytes: state.crypto.max_wrapped_key_plaintext_bytes,
  })
}

pub(crate) async fn ws_handler(
  State(state): State<AppState>,
  Path(room): Path<String>,
  ConnectInfo(addr): ConnectInfo<SocketAddr>,
  ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, ApiError> {
  let room = chat::normalize_room_name(&room).ok_or(ApiError::bad_request(
    "invalid_chat_room",
    "room name must be non-empty and contain only ASCII letters, digits, '.', '_' or '-'",
  ))?;
  let state = Arc::new(state);

  Ok(ws.on_upgrade(move |socket| chat::run_socket(state, room, addr, socket)))
}

pub(crate) async fn register_handler(
  State(state): State<AppState>,
  request: Result<Json<RegisterRequest>, JsonRejection>,
) -> Result<Json<RegisterResponse>, ApiError> {
  let Json(request) = request.map_err(map_register_request_rejection)?;
  let mut decrypted = decrypt_auth_request(&state, request)?;
  let mut password_hash = hash_password_for_storage(
    std::mem::take(&mut decrypted.payload.password),
    state.password_pepper.clone(),
  )
  .await?;
  let insert_result = insert_registered_user(
    &state.db_pool,
    &decrypted.payload.client_public_key,
    &decrypted.client_public_key_sha256,
    &password_hash,
  )
  .await;
  decrypted.payload.zeroize();
  let user_id = insert_result?;
  password_hash.zeroize();

  Ok(Json(RegisterResponse {
    status: "registered",
    user_id,
    client_public_key_sha256: decrypted.client_public_key_sha256,
  }))
}

pub(crate) async fn login_handler(
  State(state): State<AppState>,
  request: Result<Json<RegisterRequest>, JsonRejection>,
) -> Result<Json<LoginResponse>, ApiError> {
  let Json(request) = request.map_err(map_register_request_rejection)?;
  let mut decrypted = decrypt_auth_request(&state, request)?;
  let stored_user = find_user_credentials(&state.db_pool, &decrypted.client_public_key_sha256)
    .await?
    .ok_or(ApiError::unauthorized(
      "invalid_credentials",
      "invalid client_public_key or password",
    ))?;

  let password_verified = verify_password_for_login(
    std::mem::take(&mut decrypted.payload.password),
    stored_user.password_hash,
    state.password_pepper.clone(),
  )
  .await?;
  decrypted.payload.zeroize();

  if !password_verified {
    return Err(ApiError::unauthorized(
      "invalid_credentials",
      "invalid client_public_key or password",
    ));
  }

  Ok(Json(LoginResponse {
    status: "authenticated",
    user_id: stored_user.id,
    client_public_key_sha256: stored_user.client_public_key_sha256,
  }))
}

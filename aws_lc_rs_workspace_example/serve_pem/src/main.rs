use std::{
  env, fs,
  net::SocketAddr,
  path::Path,
  sync::Arc,
  time::{SystemTime, UNIX_EPOCH},
};

use aws_lc_rs::{
  encoding::{AsDer, Pkcs8V1Der, PublicKeyX509Der},
  rsa::{
    KeySize, OAEP_SHA256_MGF1SHA256, OaepPrivateDecryptingKey, OaepPublicEncryptingKey,
    PrivateDecryptingKey,
  },
};
use axum::{
  Json, Router,
  extract::State,
  http::StatusCode,
  response::{IntoResponse, Response},
  routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const PUB_KEY_FILE: &str = "public_key.der";
const PRIV_KEY_FILE: &str = "private_key.pk8";

#[derive(Clone)]
struct AppState {
  private_key_der: Arc<Vec<u8>>,
  public_key_der_base64: String,
  public_key_pem: String,
  public_key_sha256: String,
  max_plaintext_bytes: usize,
}

#[derive(Serialize)]
struct PublicKeyResponse {
  algorithm: &'static str,
  key_format: &'static str,
  public_key_pem: String,
  public_key_der_base64: String,
  sha256_hash: String,
  max_plaintext_bytes: usize,
}

#[derive(Deserialize)]
struct RegisterRequest {
  ciphertext_base64: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct RegistrationPayload {
  client_public_key: String,
  password: String,
}

#[derive(Serialize)]
struct RegisterResponse {
  status: &'static str,
  client_public_key_sha256: String,
  password_sha256: String,
}

#[derive(Serialize)]
struct ErrorResponse {
  error: String,
}

#[derive(Debug)]
enum ApiError {
  BadRequest(&'static str),
  Internal(&'static str),
}

impl IntoResponse for ApiError {
  fn into_response(self) -> Response {
    let (status, message) = match self {
      Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
      Self::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
    };

    (
      status,
      Json(ErrorResponse {
        error: message.to_owned(),
      }),
    )
      .into_response()
  }
}

fn sha256_hex(bytes: &[u8]) -> String {
  let mut hasher = Sha256::new();
  hasher.update(bytes);
  hex::encode(hasher.finalize())
}

fn der_to_pem(label: &str, der: &[u8]) -> String {
  let encoded = STANDARD.encode(der);
  let mut pem = String::new();
  pem.push_str(&format!("-----BEGIN {label}-----\n"));

  for chunk in encoded.as_bytes().chunks(64) {
    pem.push_str(std::str::from_utf8(chunk).expect("base64 output is valid utf-8"));
    pem.push('\n');
  }

  pem.push_str(&format!("-----END {label}-----\n"));
  pem
}

fn app_state_from_private_key(private_key: PrivateDecryptingKey) -> Result<AppState, String> {
  let private_key_der = AsDer::<Pkcs8V1Der>::as_der(&private_key)
    .map_err(|_| "failed to encode private key".to_owned())?
    .as_ref()
    .to_vec();

  let public_key = private_key.public_key();
  let public_key_der = AsDer::<PublicKeyX509Der>::as_der(&public_key)
    .map_err(|_| "failed to encode public key".to_owned())?
    .as_ref()
    .to_vec();

  let encrypting_key = OaepPublicEncryptingKey::new(public_key)
    .map_err(|_| "failed to prepare public encryption key".to_owned())?;

  Ok(AppState {
    private_key_der: Arc::new(private_key_der),
    public_key_der_base64: STANDARD.encode(&public_key_der),
    public_key_pem: der_to_pem("PUBLIC KEY", &public_key_der),
    public_key_sha256: sha256_hex(&public_key_der),
    max_plaintext_bytes: encrypting_key.max_plaintext_size(&OAEP_SHA256_MGF1SHA256),
  })
}

fn backup_legacy_private_key() -> Result<String, String> {
  let timestamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_err(|error| format!("failed to compute backup timestamp: {error}"))?
    .as_secs();
  let backup_path = format!("{PRIV_KEY_FILE}.legacy.{timestamp}");

  fs::rename(PRIV_KEY_FILE, &backup_path)
    .map_err(|error| format!("failed to back up legacy private key: {error}"))?;

  Ok(backup_path)
}

fn load_or_generate_app_state() -> Result<AppState, String> {
  let private_key = if Path::new(PRIV_KEY_FILE).exists() {
    let private_key_der =
      fs::read(PRIV_KEY_FILE).map_err(|error| format!("failed to read private key: {error}"))?;

    match PrivateDecryptingKey::from_pkcs8(&private_key_der) {
      Ok(private_key) => private_key,
      Err(_) => {
        let backup_path = backup_legacy_private_key()?;
        eprintln!(
          "Existing {PRIV_KEY_FILE} was not an RSA private key. Backed it up to {backup_path} and generated a new RSA keypair."
        );
        PrivateDecryptingKey::generate(KeySize::Rsa2048)
          .map_err(|_| "failed to generate RSA private key".to_owned())?
      }
    }
  } else {
    PrivateDecryptingKey::generate(KeySize::Rsa2048)
      .map_err(|_| "failed to generate RSA private key".to_owned())?
  };

  let state = app_state_from_private_key(private_key)?;

  fs::write(PRIV_KEY_FILE, state.private_key_der.as_ref())
    .map_err(|error| format!("failed to write private key: {error}"))?;
  fs::write(
    PUB_KEY_FILE,
    STANDARD.decode(&state.public_key_der_base64).unwrap(),
  )
  .map_err(|error| format!("failed to write public key: {error}"))?;

  Ok(state)
}

fn decrypt_registration_payload(
  state: &AppState,
  ciphertext: &[u8],
) -> Result<RegistrationPayload, ApiError> {
  let private_key = PrivateDecryptingKey::from_pkcs8(state.private_key_der.as_ref())
    .map_err(|_| ApiError::Internal("private key is invalid"))?;
  let decrypting_key = OaepPrivateDecryptingKey::new(private_key)
    .map_err(|_| ApiError::Internal("failed to initialize decryptor"))?;

  let mut plaintext = vec![0u8; decrypting_key.min_output_size()];
  let plaintext = decrypting_key
    .decrypt(&OAEP_SHA256_MGF1SHA256, ciphertext, &mut plaintext, None)
    .map_err(|_| ApiError::BadRequest("ciphertext could not be decrypted"))?;

  let payload: RegistrationPayload = serde_json::from_slice(plaintext)
    .map_err(|_| ApiError::BadRequest("decrypted payload is not valid JSON"))?;

  if payload.client_public_key.trim().is_empty() {
    return Err(ApiError::BadRequest("client_public_key must not be empty"));
  }

  if payload.password.trim().is_empty() {
    return Err(ApiError::BadRequest("password must not be empty"));
  }

  Ok(payload)
}

async fn get_public_key_handler(State(state): State<AppState>) -> Json<PublicKeyResponse> {
  Json(PublicKeyResponse {
    algorithm: "RSA-OAEP-256",
    key_format: "X.509 SubjectPublicKeyInfo PEM",
    public_key_pem: state.public_key_pem,
    public_key_der_base64: state.public_key_der_base64,
    sha256_hash: state.public_key_sha256,
    max_plaintext_bytes: state.max_plaintext_bytes,
  })
}

async fn register_handler(
  State(state): State<AppState>,
  Json(request): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, ApiError> {
  let ciphertext = STANDARD
    .decode(request.ciphertext_base64)
    .map_err(|_| ApiError::BadRequest("ciphertext_base64 is not valid base64"))?;

  let payload = decrypt_registration_payload(&state, &ciphertext)?;

  Ok(Json(RegisterResponse {
    status: "registered",
    client_public_key_sha256: sha256_hex(payload.client_public_key.as_bytes()),
    password_sha256: sha256_hex(payload.password.as_bytes()),
  }))
}

#[tokio::main]
async fn main() {
  let state = load_or_generate_app_state().expect("failed to initialize RSA key material");

  let app = Router::new()
    .route("/public-key", get(get_public_key_handler))
    .route("/register", post(register_handler))
    .with_state(state);

  let port = env::var("PORT")
    .ok()
    .and_then(|value| value.parse::<u16>().ok())
    .unwrap_or(3030);
  let addr = SocketAddr::from(([127, 0, 0, 1], port));

  println!("Listening on http://{addr}");
  let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
  axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
  use super::*;
  use aws_lc_rs::rsa::PublicEncryptingKey;

  #[test]
  fn pem_format_has_expected_markers() {
    let pem = der_to_pem("PUBLIC KEY", b"example");
    assert!(pem.starts_with("-----BEGIN PUBLIC KEY-----\n"));
    assert!(pem.ends_with("-----END PUBLIC KEY-----\n"));
  }

  #[test]
  fn registration_payload_round_trip_encrypts_and_decrypts() {
    let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
    let state = app_state_from_private_key(private_key.clone()).unwrap();

    let public_key_der = AsDer::<PublicKeyX509Der>::as_der(&private_key.public_key())
      .unwrap()
      .as_ref()
      .to_vec();
    let public_key = PublicEncryptingKey::from_der(&public_key_der).unwrap();
    let encrypting_key = OaepPublicEncryptingKey::new(public_key).unwrap();

    let payload = RegistrationPayload {
      client_public_key: "client-public-key".to_owned(),
      password: "correct horse battery staple".to_owned(),
    };
    let plaintext = serde_json::to_vec(&payload).unwrap();

    let mut ciphertext = vec![0u8; encrypting_key.ciphertext_size()];
    let ciphertext = encrypting_key
      .encrypt(&OAEP_SHA256_MGF1SHA256, &plaintext, &mut ciphertext, None)
      .unwrap();

    let decrypted = decrypt_registration_payload(&state, ciphertext).unwrap();
    assert_eq!(decrypted.client_public_key, payload.client_public_key);
    assert_eq!(decrypted.password, payload.password);
  }
}

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
  extract::{DefaultBodyLimit, State, rejection::JsonRejection},
  http::StatusCode,
  response::{IntoResponse, Response},
  routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

const PUB_KEY_FILE: &str = "public_key.der";
const PRIV_KEY_FILE: &str = "private_key.pk8";
const MAX_REQUEST_BODY_BYTES: usize = 16 * 1024;
const MAX_CLIENT_PUBLIC_KEY_BYTES: usize = 8 * 1024;
const MAX_PASSWORD_BYTES: usize = 1024;

#[derive(Clone)]
struct AppState {
  private_key_der: Arc<Vec<u8>>,
  public_key_der: Arc<Vec<u8>>,
  public_key_der_base64: String,
  public_key_pem: String,
  public_key_sha256: String,
  ciphertext_bytes: usize,
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
#[serde(deny_unknown_fields)]
struct RegisterRequest {
  ciphertext_base64: String,
}

#[derive(Debug, Deserialize, Serialize, Zeroize)]
#[zeroize(drop)]
#[serde(deny_unknown_fields)]
struct RegistrationPayload {
  client_public_key: String,
  password: String,
}

#[derive(Debug, Serialize)]
struct RegisterResponse {
  status: &'static str,
  client_public_key_sha256: String,
}

#[derive(Serialize)]
struct ErrorResponse {
  code: &'static str,
  error: String,
}

#[derive(Debug, Clone, Copy)]
struct ApiError {
  status: StatusCode,
  code: &'static str,
  message: &'static str,
}

impl ApiError {
  const fn bad_request(code: &'static str, message: &'static str) -> Self {
    Self {
      status: StatusCode::BAD_REQUEST,
      code,
      message,
    }
  }

  const fn unsupported_media_type(code: &'static str, message: &'static str) -> Self {
    Self {
      status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
      code,
      message,
    }
  }

  const fn payload_too_large(code: &'static str, message: &'static str) -> Self {
    Self {
      status: StatusCode::PAYLOAD_TOO_LARGE,
      code,
      message,
    }
  }

  const fn internal(code: &'static str, message: &'static str) -> Self {
    Self {
      status: StatusCode::INTERNAL_SERVER_ERROR,
      code,
      message,
    }
  }
}

impl IntoResponse for ApiError {
  fn into_response(self) -> Response {
    (
      self.status,
      Json(ErrorResponse {
        code: self.code,
        error: self.message.to_owned(),
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
    public_key_der: Arc::new(public_key_der.clone()),
    public_key_der_base64: STANDARD.encode(&public_key_der),
    public_key_pem: der_to_pem("PUBLIC KEY", &public_key_der),
    public_key_sha256: sha256_hex(&public_key_der),
    ciphertext_bytes: encrypting_key.ciphertext_size(),
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
  fs::write(PUB_KEY_FILE, state.public_key_der.as_ref())
    .map_err(|error| format!("failed to write public key: {error}"))?;

  Ok(state)
}

fn map_register_request_rejection(rejection: JsonRejection) -> ApiError {
  match rejection {
    JsonRejection::JsonDataError(_) | JsonRejection::JsonSyntaxError(_) => {
      ApiError::bad_request("invalid_request_json", "request body must be valid JSON")
    }
    JsonRejection::MissingJsonContentType(_) => ApiError::unsupported_media_type(
      "missing_json_content_type",
      "Content-Type must be application/json",
    ),
    JsonRejection::BytesRejection(_) => ApiError::bad_request(
      "invalid_request_body",
      "request body could not be read",
    ),
    _ => ApiError::payload_too_large(
      "request_body_too_large",
      "request body exceeds the allowed size",
    ),
  }
}

fn decrypt_registration_payload(
  state: &AppState,
  ciphertext: &[u8],
) -> Result<RegistrationPayload, ApiError> {
  if ciphertext.len() != state.ciphertext_bytes {
    return Err(ApiError::bad_request(
      "invalid_ciphertext_length",
      "ciphertext length does not match the RSA key size",
    ));
  }

  let private_key = PrivateDecryptingKey::from_pkcs8(state.private_key_der.as_ref())
    .map_err(|_| ApiError::internal("invalid_private_key", "private key is invalid"))?;
  let decrypting_key = OaepPrivateDecryptingKey::new(private_key)
    .map_err(|_| ApiError::internal("decryptor_init_failed", "failed to initialize decryptor"))?;

  let mut plaintext = vec![0u8; decrypting_key.min_output_size()];
  let plaintext_len = decrypting_key
    .decrypt(&OAEP_SHA256_MGF1SHA256, ciphertext, &mut plaintext, None)
    .map(|plaintext| plaintext.len())
    .map_err(|_| ApiError::bad_request("ciphertext_decryption_failed", "ciphertext could not be decrypted"))?;

  let payload: RegistrationPayload = serde_json::from_slice(&plaintext[..plaintext_len]).map_err(|_| {
    ApiError::bad_request(
      "invalid_registration_json",
      "decrypted payload is not valid JSON",
    )
  })?;
  plaintext.zeroize();

  if payload.client_public_key.trim().is_empty() {
    return Err(ApiError::bad_request(
      "empty_client_public_key",
      "client_public_key must not be empty",
    ));
  }

  if payload.client_public_key.len() > MAX_CLIENT_PUBLIC_KEY_BYTES {
    return Err(ApiError::payload_too_large(
      "client_public_key_too_large",
      "client_public_key exceeds the allowed size",
    ));
  }

  if payload.password.trim().is_empty() {
    return Err(ApiError::bad_request(
      "empty_password",
      "password must not be empty",
    ));
  }

  if payload.password.len() > MAX_PASSWORD_BYTES {
    return Err(ApiError::payload_too_large(
      "password_too_large",
      "password exceeds the allowed size",
    ));
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
  request: Result<Json<RegisterRequest>, JsonRejection>,
) -> Result<Json<RegisterResponse>, ApiError> {
  let Json(request) = request.map_err(map_register_request_rejection)?;
  let ciphertext = STANDARD
    .decode(request.ciphertext_base64)
    .map_err(|_| ApiError::bad_request("invalid_ciphertext_base64", "ciphertext_base64 is not valid base64"))?;

  let mut payload = decrypt_registration_payload(&state, &ciphertext)?;
  let client_public_key_sha256 = sha256_hex(payload.client_public_key.as_bytes());
  payload.zeroize();

  Ok(Json(RegisterResponse {
    status: "registered",
    client_public_key_sha256,
  }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let state = load_or_generate_app_state().map_err(std::io::Error::other)?;

  let app = Router::new()
    .route("/public-key", get(get_public_key_handler))
    .route("/register", post(register_handler))
    .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
    .with_state(state);

  let port = env::var("PORT")
    .ok()
    .and_then(|value| value.parse::<u16>().ok())
    .unwrap_or(3030);
  let addr = SocketAddr::from(([127, 0, 0, 1], port));

  println!("Listening on http://{addr}");
  let listener = tokio::net::TcpListener::bind(&addr).await?;
  axum::serve(listener, app).await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use aws_lc_rs::rsa::PublicEncryptingKey;

  fn encrypt_bytes(private_key: &PrivateDecryptingKey, plaintext: &[u8]) -> Vec<u8> {
    let public_key_der = AsDer::<PublicKeyX509Der>::as_der(&private_key.public_key())
      .unwrap()
      .as_ref()
      .to_vec();
    let public_key = PublicEncryptingKey::from_der(&public_key_der).unwrap();
    let encrypting_key = OaepPublicEncryptingKey::new(public_key).unwrap();
    let mut ciphertext = vec![0u8; encrypting_key.ciphertext_size()];

    encrypting_key
      .encrypt(&OAEP_SHA256_MGF1SHA256, plaintext, &mut ciphertext, None)
      .unwrap()
      .to_vec()
  }

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

    let payload = RegistrationPayload {
      client_public_key: "client-public-key".to_owned(),
      password: "correct horse battery staple".to_owned(),
    };
    let plaintext = serde_json::to_vec(&payload).unwrap();

    let ciphertext = encrypt_bytes(&private_key, &plaintext);

    let decrypted = decrypt_registration_payload(&state, &ciphertext).unwrap();
    assert_eq!(decrypted.client_public_key, payload.client_public_key);
    assert_eq!(decrypted.password, payload.password);
  }

  #[test]
  fn registration_payload_rejects_wrong_ciphertext_size() {
    let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
    let state = app_state_from_private_key(private_key).unwrap();

    let err = decrypt_registration_payload(&state, &[0u8; 32]).unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.code, "invalid_ciphertext_length");
  }

  #[test]
  fn registration_payload_rejects_invalid_json() {
    let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
    let state = app_state_from_private_key(private_key.clone()).unwrap();
    let ciphertext = encrypt_bytes(&private_key, b"not-json");

    let err = decrypt_registration_payload(&state, &ciphertext).unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.code, "invalid_registration_json");
  }

  #[test]
  fn registration_payload_rejects_empty_fields() {
    let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
    let state = app_state_from_private_key(private_key.clone()).unwrap();
    let plaintext = serde_json::to_vec(&RegistrationPayload {
      client_public_key: "   ".to_owned(),
      password: "secret".to_owned(),
    })
    .unwrap();
    let ciphertext = encrypt_bytes(&private_key, &plaintext);

    let err = decrypt_registration_payload(&state, &ciphertext).unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.code, "empty_client_public_key");
  }

  #[test]
  fn registration_payload_rejects_tampered_ciphertext() {
    let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
    let state = app_state_from_private_key(private_key.clone()).unwrap();
    let plaintext = serde_json::to_vec(&RegistrationPayload {
      client_public_key: "client-public-key".to_owned(),
      password: "correct horse battery staple".to_owned(),
    })
    .unwrap();
    let mut ciphertext = encrypt_bytes(&private_key, &plaintext);
    ciphertext[0] ^= 0x01;

    let err = decrypt_registration_payload(&state, &ciphertext).unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.code, "ciphertext_decryption_failed");
  }

  #[tokio::test]
  async fn register_handler_rejects_invalid_base64() {
    let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
    let state = app_state_from_private_key(private_key).unwrap();

    let err = register_handler(
      State(state),
      Ok(Json(RegisterRequest {
        ciphertext_base64: "%%%".to_owned(),
      })),
    )
    .await
    .unwrap_err();

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.code, "invalid_ciphertext_base64");
  }
}

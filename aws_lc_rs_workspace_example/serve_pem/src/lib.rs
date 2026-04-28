use std::{env, error::Error, net::SocketAddr, sync::Arc};

use axum::{
  Router,
  extract::DefaultBodyLimit,
  routing::{get, post},
};
use sqlx::{PgPool, postgres::PgPoolOptions};

mod chat;
mod crypto;
mod db;
mod error;
mod handlers;
mod models;
mod password;
mod service;
mod tls;

pub use crypto::{CryptoState, crypto_state_from_private_key, ensure_crypto_files};
pub use password::hash_password;

pub const MAX_REQUEST_BODY_BYTES: usize = 16 * 1024;
pub const MAX_CLIENT_PUBLIC_KEY_BYTES: usize = 8 * 1024;
pub const MAX_PASSWORD_BYTES: usize = 1024;
pub const CONTENT_ENCRYPTION_KEY_BYTES: usize = 32;
pub const CONTENT_ENCRYPTION_TAG_BYTES: usize = 16;
pub const PASSWORD_PEPPER_ENV: &str = "PASSWORD_PEPPER";
pub const PASSWORD_HASH_MEMORY_COST_KIB: u32 = 19_456;
pub const PASSWORD_HASH_ITERATIONS: u32 = 2;
pub const PASSWORD_HASH_PARALLELISM: u32 = 1;
pub const PASSWORD_HASH_LENGTH: usize = 32;
pub const MIN_PASSWORD_PEPPER_BYTES: usize = 16;
pub const PUB_KEY_FILE: &str = "public_key.der";
pub const PRIV_KEY_FILE: &str = "private_key.pk8";
pub const MAX_CHAT_ROOM_NAME_BYTES: usize = 64;
pub const MAX_CHAT_USER_NAME_BYTES: usize = 64;
pub const MAX_CHAT_MESSAGE_BYTES: usize = 2048;

#[derive(Clone)]
pub struct AppState {
  pub(crate) crypto: Arc<CryptoState>,
  pub(crate) db_pool: PgPool,
  pub(crate) password_pepper: Option<Arc<Vec<u8>>>,
  pub(crate) chat: Arc<chat::ChatState>,
}

impl AppState {
  pub fn new_for_test(
    crypto: CryptoState,
    db_pool: PgPool,
    password_pepper: Option<Vec<u8>>,
  ) -> Self {
    Self {
      crypto: Arc::new(crypto),
      db_pool,
      password_pepper: password_pepper.map(Arc::new),
      chat: Arc::new(chat::ChatState::new()),
    }
  }
}

pub fn build_router(state: AppState) -> Router {
  Router::new()
    .route("/public-key", get(handlers::get_public_key_handler))
    .route("/register", post(handlers::register_handler))
    .route("/login", post(handlers::login_handler))
    .route("/ws/{room}", get(handlers::ws_handler))
    .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
    .with_state(state)
}

async fn init_state_from_env() -> Result<AppState, Box<dyn Error>> {
  let _ = dotenvy::dotenv();
  let database_url =
    env::var("DATABASE_URL").map_err(|_| std::io::Error::other("DATABASE_URL must be set"))?;
  let db_pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&database_url)
    .await?;
  sqlx::migrate!("./migrations").run(&db_pool).await?;

  let crypto = crypto::load_crypto_state().map_err(std::io::Error::other)?;
  let password_pepper = password::load_password_pepper().map_err(std::io::Error::other)?;

  Ok(AppState::new_for_test(crypto, db_pool, password_pepper))
}

pub async fn run() -> Result<(), Box<dyn Error>> {
  let state = init_state_from_env().await?;
  let port = env::var("PORT")
    .ok()
    .and_then(|value| value.parse::<u16>().ok())
    .unwrap_or(3030);
  let addr = SocketAddr::from(([127, 0, 0, 1], port));
  let tls_config = tls::rustls_config_from_crypto_state(state.crypto.as_ref()).await?;

  println!("Listening on https://{addr}");
  axum_server::bind_rustls(addr, tls_config)
    .serve(build_router(state).into_make_service_with_connect_info::<SocketAddr>())
    .await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use argon2::{
    Argon2, PasswordVerifier,
    password_hash::{
      PasswordHash,
      rand_core::{OsRng, RngCore},
    },
  };
  use aws_lc_rs::{
    aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    encoding::{AsDer, PublicKeyX509Der},
    rsa::{
      KeySize, OAEP_SHA256_MGF1SHA256, OaepPublicEncryptingKey, PrivateDecryptingKey,
      PublicEncryptingKey,
    },
  };
  use axum::{Json, extract::State, http::StatusCode};
  use base64::{Engine as _, engine::general_purpose::STANDARD};
  use zeroize::Zeroize;

  use super::*;

  fn encrypt_registration_payload(
    private_key: &PrivateDecryptingKey,
    plaintext: &[u8],
  ) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let public_key_der = AsDer::<PublicKeyX509Der>::as_der(&private_key.public_key())
      .unwrap()
      .as_ref()
      .to_vec();
    let public_key = PublicEncryptingKey::from_der(&public_key_der).unwrap();
    let rsa_encrypting_key = OaepPublicEncryptingKey::new(public_key).unwrap();
    let mut content_key = [0u8; CONTENT_ENCRYPTION_KEY_BYTES];
    OsRng.fill_bytes(&mut content_key);
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);

    let unbound_key = UnboundKey::new(&AES_256_GCM, &content_key).unwrap();
    let aead_key = LessSafeKey::new(unbound_key);
    let mut ciphertext = plaintext.to_vec();
    aead_key
      .seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::empty(),
        &mut ciphertext,
      )
      .unwrap();

    let mut wrapped_key = vec![0u8; rsa_encrypting_key.ciphertext_size()];
    let wrapped_key = rsa_encrypting_key
      .encrypt(
        &OAEP_SHA256_MGF1SHA256,
        &content_key,
        &mut wrapped_key,
        None,
      )
      .unwrap()
      .to_vec();
    content_key.zeroize();

    (wrapped_key, nonce.to_vec(), ciphertext)
  }

  #[test]
  fn pem_format_has_expected_markers() {
    let pem = crypto::der_to_pem("PUBLIC KEY", b"example");
    assert!(pem.starts_with("-----BEGIN PUBLIC KEY-----\n"));
    assert!(pem.ends_with("-----END PUBLIC KEY-----\n"));
  }

  #[test]
  fn registration_payload_round_trip_encrypts_and_decrypts() {
    let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
    let state = crypto_state_from_private_key(private_key.clone()).unwrap();

    let payload = models::RegistrationPayload {
      client_public_key: "client-public-key".to_owned(),
      password: "correct horse battery staple".to_owned(),
    };
    let plaintext = serde_json::to_vec(&payload).unwrap();

    let (wrapped_key, nonce, ciphertext) = encrypt_registration_payload(&private_key, &plaintext);

    let decrypted =
      crypto::decrypt_registration_payload(&state, &wrapped_key, &nonce, &ciphertext).unwrap();
    assert_eq!(decrypted.client_public_key, payload.client_public_key);
    assert_eq!(decrypted.password, payload.password);
  }

  #[test]
  fn registration_payload_rejects_wrong_wrapped_key_size() {
    let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
    let state = crypto_state_from_private_key(private_key).unwrap();
    let nonce = vec![0u8; NONCE_LEN];
    let ciphertext = vec![0u8; CONTENT_ENCRYPTION_TAG_BYTES];

    let err =
      crypto::decrypt_registration_payload(&state, &[0u8; 32], &nonce, &ciphertext).unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.code, "invalid_wrapped_key_length");
  }

  #[test]
  fn registration_payload_rejects_invalid_json() {
    let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
    let state = crypto_state_from_private_key(private_key.clone()).unwrap();
    let (wrapped_key, nonce, ciphertext) = encrypt_registration_payload(&private_key, b"not-json");

    let err =
      crypto::decrypt_registration_payload(&state, &wrapped_key, &nonce, &ciphertext).unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.code, "invalid_registration_json");
  }

  #[test]
  fn registration_payload_rejects_empty_fields() {
    let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
    let state = crypto_state_from_private_key(private_key.clone()).unwrap();
    let plaintext = serde_json::to_vec(&models::RegistrationPayload {
      client_public_key: "   ".to_owned(),
      password: "secret".to_owned(),
    })
    .unwrap();
    let (wrapped_key, nonce, ciphertext) = encrypt_registration_payload(&private_key, &plaintext);

    let err =
      crypto::decrypt_registration_payload(&state, &wrapped_key, &nonce, &ciphertext).unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.code, "empty_client_public_key");
  }

  #[test]
  fn registration_payload_rejects_tampered_ciphertext() {
    let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
    let state = crypto_state_from_private_key(private_key.clone()).unwrap();
    let plaintext = serde_json::to_vec(&models::RegistrationPayload {
      client_public_key: "client-public-key".to_owned(),
      password: "correct horse battery staple".to_owned(),
    })
    .unwrap();
    let (wrapped_key, nonce, mut ciphertext) =
      encrypt_registration_payload(&private_key, &plaintext);
    ciphertext[0] ^= 0x01;

    let err =
      crypto::decrypt_registration_payload(&state, &wrapped_key, &nonce, &ciphertext).unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.code, "ciphertext_decryption_failed");
  }

  #[test]
  fn registration_payload_rejects_invalid_nonce_length() {
    let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
    let state = crypto_state_from_private_key(private_key.clone()).unwrap();
    let plaintext = serde_json::to_vec(&models::RegistrationPayload {
      client_public_key: "client-public-key".to_owned(),
      password: "correct horse battery staple".to_owned(),
    })
    .unwrap();
    let (wrapped_key, _nonce, ciphertext) = encrypt_registration_payload(&private_key, &plaintext);

    let err = crypto::decrypt_registration_payload(&state, &wrapped_key, &[0u8; 8], &ciphertext)
      .unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.code, "invalid_nonce_length");
  }

  #[tokio::test]
  async fn register_handler_rejects_invalid_base64() {
    let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
    let crypto = crypto_state_from_private_key(private_key).unwrap();
    let db_pool = PgPoolOptions::new()
      .connect_lazy("postgres://postgres:postgres@localhost/postgres")
      .unwrap();
    let state = AppState::new_for_test(crypto, db_pool, None);

    let err = handlers::register_handler(
      State(state),
      Ok(Json(models::RegisterRequest {
        wrapped_key_base64: "%%%".to_owned(),
        nonce_base64: STANDARD.encode([0u8; NONCE_LEN]),
        ciphertext_base64: "%%%".to_owned(),
      })),
    )
    .await
    .unwrap_err();

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.code, "invalid_wrapped_key_base64");
  }

  #[tokio::test]
  async fn login_handler_rejects_invalid_base64() {
    let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
    let crypto = crypto_state_from_private_key(private_key).unwrap();
    let db_pool = PgPoolOptions::new()
      .connect_lazy("postgres://postgres:postgres@localhost/postgres")
      .unwrap();
    let state = AppState::new_for_test(crypto, db_pool, None);

    let err = handlers::login_handler(
      State(state),
      Ok(Json(models::RegisterRequest {
        wrapped_key_base64: "%%%".to_owned(),
        nonce_base64: STANDARD.encode([0u8; NONCE_LEN]),
        ciphertext_base64: "%%%".to_owned(),
      })),
    )
    .await
    .unwrap_err();

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.code, "invalid_wrapped_key_base64");
  }

  #[test]
  fn password_hash_uses_argon2id_and_verifies() {
    let password_hash = hash_password("correct horse battery staple", None).unwrap();
    let parsed_hash = PasswordHash::new(&password_hash).unwrap();

    assert_eq!(parsed_hash.algorithm.as_str(), "argon2id");
    Argon2::default()
      .verify_password("correct horse battery staple".as_bytes(), &parsed_hash)
      .unwrap();
  }

  #[test]
  fn verify_password_accepts_matching_password() {
    let password_hash = hash_password("correct horse battery staple", None).unwrap();
    assert!(
      password::verify_password("correct horse battery staple", &password_hash, None).unwrap()
    );
  }

  #[test]
  fn verify_password_rejects_wrong_password() {
    let password_hash = hash_password("correct horse battery staple", None).unwrap();
    assert!(!password::verify_password("wrong password", &password_hash, None).unwrap());
  }
}

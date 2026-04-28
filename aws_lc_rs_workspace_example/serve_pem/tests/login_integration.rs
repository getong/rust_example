use argon2::password_hash::rand_core::{OsRng, RngCore};
use aws_lc_rs::{
  aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
  encoding::{AsDer, PublicKeyX509Der},
  rsa::{
    KeySize, OAEP_SHA256_MGF1SHA256, OaepPublicEncryptingKey, PrivateDecryptingKey,
    PublicEncryptingKey,
  },
};
use axum::{
  body::Body,
  http::{Request, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use serve_pem::{
  AppState, CONTENT_ENCRYPTION_KEY_BYTES, build_router, crypto_state_from_private_key,
  hash_password,
};
use sha2::Digest;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::util::ServiceExt;
use zeroize::Zeroize;

fn local_test_database_url() -> String {
  std::env::var("DATABASE_URL")
    .unwrap_or_else(|_| "postgres://postgres:PgSuper2026!@localhost:5432/serve_pem".to_owned())
}

async fn local_test_db_pool() -> PgPool {
  let database_url = local_test_database_url();
  let pool = PgPoolOptions::new()
    .max_connections(1)
    .connect(&database_url)
    .await
    .expect("failed to connect to local PostgreSQL for integration test");

  sqlx::migrate!("./migrations")
    .run(&pool)
    .await
    .expect("failed to run migrations for integration test");

  pool
}

fn unique_test_client_public_key() -> String {
  let mut random_bytes = [0u8; 16];
  OsRng.fill_bytes(&mut random_bytes);
  format!("integration-client-{}", hex::encode(random_bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
  let mut hasher = sha2::Sha256::new();
  sha2::Digest::update(&mut hasher, bytes);
  hex::encode(sha2::Digest::finalize(hasher))
}

async fn insert_integration_test_user(
  pool: &PgPool,
  client_public_key: &str,
  password: &str,
) -> (i64, String) {
  let client_public_key_sha256 = sha256_hex(client_public_key.as_bytes());
  let password_hash =
    hash_password(password, None).expect("failed to hash integration test password");
  let user_id = sqlx::query_scalar::<_, i64>(
    r#"
    INSERT INTO users (client_public_key, client_public_key_sha256, password_hash)
    VALUES ($1, $2, $3)
    RETURNING id
    "#,
  )
  .bind(client_public_key)
  .bind(&client_public_key_sha256)
  .bind(password_hash)
  .fetch_one(pool)
  .await
  .expect("failed to insert integration test user");

  (user_id, client_public_key_sha256)
}

async fn delete_integration_test_user(pool: &PgPool, user_id: i64) {
  sqlx::query("DELETE FROM users WHERE id = $1")
    .bind(user_id)
    .execute(pool)
    .await
    .expect("failed to delete integration test user");
}

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

async fn post_login_request(router: axum::Router, request_body: Value) -> (StatusCode, Value) {
  let response = router
    .oneshot(
      Request::post("/login")
        .header("content-type", "application/json")
        .body(Body::from(request_body.to_string()))
        .unwrap(),
    )
    .await
    .expect("request to /login should succeed");
  let status = response.status();
  let body = axum::body::to_bytes(response.into_body(), usize::MAX)
    .await
    .expect("response body should be readable");
  let json = serde_json::from_slice(&body).expect("response should be valid JSON");

  (status, json)
}

#[tokio::test]
async fn login_handler_authenticates_against_local_postgres() {
  let db_pool = local_test_db_pool().await;
  let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
  let crypto = crypto_state_from_private_key(private_key.clone()).unwrap();
  let client_public_key = unique_test_client_public_key();
  let password = "correct horse battery staple";
  let (user_id, client_public_key_sha256) =
    insert_integration_test_user(&db_pool, &client_public_key, password).await;

  let result = async {
    let app = build_router(AppState::new_for_test(crypto, db_pool.clone(), None));
    let plaintext = serde_json::to_vec(&json!({
      "client_public_key": client_public_key,
      "password": password,
    }))
    .unwrap();
    let (wrapped_key, nonce, ciphertext) = encrypt_registration_payload(&private_key, &plaintext);

    post_login_request(
      app,
      json!({
        "wrapped_key_base64": STANDARD.encode(wrapped_key),
        "nonce_base64": STANDARD.encode(nonce),
        "ciphertext_base64": STANDARD.encode(ciphertext),
      }),
    )
    .await
  }
  .await;

  delete_integration_test_user(&db_pool, user_id).await;

  let (status, body) = result;
  assert_eq!(status, StatusCode::OK);
  assert_eq!(body["status"], "authenticated");
  assert_eq!(body["user_id"], user_id);
  assert_eq!(body["client_public_key_sha256"], client_public_key_sha256);
}

#[tokio::test]
async fn login_handler_rejects_wrong_password_against_local_postgres() {
  let db_pool = local_test_db_pool().await;
  let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
  let crypto = crypto_state_from_private_key(private_key.clone()).unwrap();
  let client_public_key = unique_test_client_public_key();
  let (user_id, _client_public_key_sha256) =
    insert_integration_test_user(&db_pool, &client_public_key, "correct horse battery staple")
      .await;

  let result = async {
    let app = build_router(AppState::new_for_test(crypto, db_pool.clone(), None));
    let plaintext = serde_json::to_vec(&json!({
      "client_public_key": client_public_key,
      "password": "wrong password",
    }))
    .unwrap();
    let (wrapped_key, nonce, ciphertext) = encrypt_registration_payload(&private_key, &plaintext);

    post_login_request(
      app,
      json!({
        "wrapped_key_base64": STANDARD.encode(wrapped_key),
        "nonce_base64": STANDARD.encode(nonce),
        "ciphertext_base64": STANDARD.encode(ciphertext),
      }),
    )
    .await
  }
  .await;

  delete_integration_test_user(&db_pool, user_id).await;

  let (status, body) = result;
  assert_eq!(status, StatusCode::UNAUTHORIZED);
  assert_eq!(body["code"], "invalid_credentials");
}

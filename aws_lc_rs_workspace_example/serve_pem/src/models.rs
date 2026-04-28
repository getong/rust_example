use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

#[derive(Serialize, Deserialize)]
pub(crate) struct PublicKeyResponse {
  pub(crate) transport: &'static str,
  pub(crate) key_encryption_algorithm: &'static str,
  pub(crate) content_encryption_algorithm: &'static str,
  pub(crate) key_format: &'static str,
  pub(crate) public_key_pem: String,
  pub(crate) public_key_der_base64: String,
  pub(crate) sha256_hash: String,
  pub(crate) wrapped_key_bytes: usize,
  pub(crate) nonce_bytes: usize,
  pub(crate) max_wrapped_key_plaintext_bytes: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegisterRequest {
  pub(crate) wrapped_key_base64: String,
  pub(crate) nonce_base64: String,
  pub(crate) ciphertext_base64: String,
}

#[derive(Debug, Deserialize, Serialize, Zeroize)]
#[zeroize(drop)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegistrationPayload {
  pub(crate) client_public_key: String,
  pub(crate) password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RegisterResponse {
  pub(crate) status: &'static str,
  pub(crate) user_id: i64,
  pub(crate) client_public_key_sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct LoginResponse {
  pub(crate) status: &'static str,
  pub(crate) user_id: i64,
  pub(crate) client_public_key_sha256: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ErrorResponse {
  pub(crate) code: &'static str,
  pub(crate) error: String,
}

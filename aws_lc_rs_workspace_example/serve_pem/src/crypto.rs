use std::{
  convert::TryInto,
  fs,
  path::Path,
  sync::Arc,
  time::{SystemTime, UNIX_EPOCH},
};

use aws_lc_rs::{
  aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
  encoding::{AsDer, Pkcs8V1Der, PublicKeyX509Der},
  rsa::{
    KeySize, OAEP_SHA256_MGF1SHA256, OaepPrivateDecryptingKey, OaepPublicEncryptingKey,
    PrivateDecryptingKey,
  },
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use crate::{
  CONTENT_ENCRYPTION_KEY_BYTES, CONTENT_ENCRYPTION_TAG_BYTES, MAX_CLIENT_PUBLIC_KEY_BYTES,
  MAX_PASSWORD_BYTES, PRIV_KEY_FILE, PUB_KEY_FILE, error::ApiError, models::RegistrationPayload,
};

#[derive(Clone)]
pub struct CryptoState {
  pub(crate) private_key_der: Arc<Vec<u8>>,
  pub(crate) public_key_der: Arc<Vec<u8>>,
  pub(crate) public_key_der_base64: String,
  pub(crate) public_key_pem: String,
  pub(crate) public_key_sha256: String,
  pub(crate) wrapped_key_bytes: usize,
  pub(crate) max_wrapped_key_plaintext_bytes: usize,
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
  let mut hasher = Sha256::new();
  hasher.update(bytes);
  hex::encode(hasher.finalize())
}

pub(crate) fn der_to_pem(label: &str, der: &[u8]) -> String {
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

pub fn crypto_state_from_private_key(
  private_key: PrivateDecryptingKey,
) -> Result<CryptoState, String> {
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

  Ok(CryptoState {
    private_key_der: Arc::new(private_key_der),
    public_key_der: Arc::new(public_key_der.clone()),
    public_key_der_base64: STANDARD.encode(&public_key_der),
    public_key_pem: der_to_pem("PUBLIC KEY", &public_key_der),
    public_key_sha256: sha256_hex(&public_key_der),
    wrapped_key_bytes: encrypting_key.ciphertext_size(),
    max_wrapped_key_plaintext_bytes: encrypting_key.max_plaintext_size(&OAEP_SHA256_MGF1SHA256),
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

pub fn ensure_crypto_files() -> Result<CryptoState, String> {
  let private_key = if Path::new(PRIV_KEY_FILE).exists() {
    let private_key_der =
      fs::read(PRIV_KEY_FILE).map_err(|error| format!("failed to read private key: {error}"))?;

    match PrivateDecryptingKey::from_pkcs8(&private_key_der) {
      Ok(private_key) => private_key,
      Err(_) => {
        let backup_path = backup_legacy_private_key()?;
        eprintln!(
          "Existing {PRIV_KEY_FILE} was not an RSA private key. Backed it up to {backup_path} and \
           generated a new RSA keypair."
        );
        PrivateDecryptingKey::generate(KeySize::Rsa2048)
          .map_err(|_| "failed to generate RSA private key".to_owned())?
      }
    }
  } else {
    PrivateDecryptingKey::generate(KeySize::Rsa2048)
      .map_err(|_| "failed to generate RSA private key".to_owned())?
  };

  let state = crypto_state_from_private_key(private_key)?;

  fs::write(PRIV_KEY_FILE, state.private_key_der.as_ref())
    .map_err(|error| format!("failed to write private key: {error}"))?;
  fs::write(PUB_KEY_FILE, state.public_key_der.as_ref())
    .map_err(|error| format!("failed to write public key: {error}"))?;

  Ok(state)
}

pub(crate) fn load_crypto_state() -> Result<CryptoState, String> {
  let private_key_der =
    fs::read(PRIV_KEY_FILE).map_err(|error| format!("failed to read {PRIV_KEY_FILE}: {error}"))?;
  let public_key_der =
    fs::read(PUB_KEY_FILE).map_err(|error| format!("failed to read {PUB_KEY_FILE}: {error}"))?;

  let private_key = PrivateDecryptingKey::from_pkcs8(&private_key_der)
    .map_err(|_| format!("{PRIV_KEY_FILE} is not a valid RSA PKCS#8 private key"))?;
  let state = crypto_state_from_private_key(private_key)?;

  if state.public_key_der.as_ref().as_slice() != public_key_der.as_slice() {
    return Err(format!(
      "{PUB_KEY_FILE} does not match {PRIV_KEY_FILE}; run `cargo run --bin generate_keypair` to \
       regenerate the key files"
    ));
  }

  Ok(state)
}

fn decrypt_wrapped_content_key(
  state: &CryptoState,
  wrapped_key: &[u8],
) -> Result<[u8; CONTENT_ENCRYPTION_KEY_BYTES], ApiError> {
  if wrapped_key.len() != state.wrapped_key_bytes {
    return Err(ApiError::bad_request(
      "invalid_wrapped_key_length",
      "wrapped_key length does not match the RSA key size",
    ));
  }

  let private_key = PrivateDecryptingKey::from_pkcs8(state.private_key_der.as_ref())
    .map_err(|_| ApiError::internal("invalid_private_key", "private key is invalid"))?;
  let decrypting_key = OaepPrivateDecryptingKey::new(private_key)
    .map_err(|_| ApiError::internal("decryptor_init_failed", "failed to initialize decryptor"))?;

  let mut plaintext = vec![0u8; decrypting_key.min_output_size()];
  let plaintext_len = decrypting_key
    .decrypt(&OAEP_SHA256_MGF1SHA256, wrapped_key, &mut plaintext, None)
    .map(|plaintext| plaintext.len())
    .map_err(|_| {
      ApiError::bad_request(
        "wrapped_key_decryption_failed",
        "wrapped_key could not be decrypted",
      )
    })?;

  if plaintext_len != CONTENT_ENCRYPTION_KEY_BYTES {
    plaintext.zeroize();
    return Err(ApiError::bad_request(
      "invalid_wrapped_key_plaintext_length",
      "decrypted wrapped_key length is invalid",
    ));
  }

  let mut content_key = [0u8; CONTENT_ENCRYPTION_KEY_BYTES];
  content_key.copy_from_slice(&plaintext[.. plaintext_len]);
  plaintext.zeroize();

  Ok(content_key)
}

pub(crate) fn decrypt_registration_payload(
  state: &CryptoState,
  wrapped_key: &[u8],
  nonce_bytes: &[u8],
  ciphertext: &[u8],
) -> Result<RegistrationPayload, ApiError> {
  if nonce_bytes.len() != NONCE_LEN {
    return Err(ApiError::bad_request(
      "invalid_nonce_length",
      "nonce length does not match AES-GCM requirements",
    ));
  }

  if ciphertext.len() < CONTENT_ENCRYPTION_TAG_BYTES {
    return Err(ApiError::bad_request(
      "invalid_ciphertext_length",
      "ciphertext is too short for AES-GCM",
    ));
  }

  let mut content_key = decrypt_wrapped_content_key(state, wrapped_key)?;
  let unbound_key = UnboundKey::new(&AES_256_GCM, &content_key)
    .map_err(|_| ApiError::internal("invalid_content_key", "decrypted content key is invalid"))?;
  let key = LessSafeKey::new(unbound_key);
  let nonce = Nonce::assume_unique_for_key(nonce_bytes.try_into().map_err(|_| {
    ApiError::bad_request(
      "invalid_nonce_length",
      "nonce length does not match AES-GCM requirements",
    )
  })?);

  let mut plaintext = ciphertext.to_vec();
  let plaintext = key
    .open_in_place(nonce, Aad::empty(), &mut plaintext)
    .map_err(|_| {
      ApiError::bad_request(
        "ciphertext_decryption_failed",
        "ciphertext could not be decrypted",
      )
    })?;

  let payload: RegistrationPayload = serde_json::from_slice(plaintext).map_err(|_| {
    ApiError::bad_request(
      "invalid_registration_json",
      "decrypted payload is not valid JSON",
    )
  })?;
  plaintext.zeroize();
  content_key.zeroize();

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

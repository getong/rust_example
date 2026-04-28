use base64::{Engine as _, engine::general_purpose::STANDARD};

use crate::{
  AppState,
  crypto::{decrypt_registration_payload, sha256_hex},
  error::ApiError,
  models::{RegisterRequest, RegistrationPayload},
};

pub(crate) struct DecryptedAuthPayload {
  pub(crate) payload: RegistrationPayload,
  pub(crate) client_public_key_sha256: String,
}

fn decode_encrypted_request(
  request: RegisterRequest,
) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), ApiError> {
  let wrapped_key = STANDARD.decode(request.wrapped_key_base64).map_err(|_| {
    ApiError::bad_request(
      "invalid_wrapped_key_base64",
      "wrapped_key_base64 is not valid base64",
    )
  })?;
  let nonce = STANDARD.decode(request.nonce_base64).map_err(|_| {
    ApiError::bad_request("invalid_nonce_base64", "nonce_base64 is not valid base64")
  })?;
  let ciphertext = STANDARD.decode(request.ciphertext_base64).map_err(|_| {
    ApiError::bad_request(
      "invalid_ciphertext_base64",
      "ciphertext_base64 is not valid base64",
    )
  })?;

  Ok((wrapped_key, nonce, ciphertext))
}

pub(crate) fn decrypt_auth_request(
  state: &AppState,
  request: RegisterRequest,
) -> Result<DecryptedAuthPayload, ApiError> {
  let (wrapped_key, nonce, ciphertext) = decode_encrypted_request(request)?;
  let payload =
    decrypt_registration_payload(state.crypto.as_ref(), &wrapped_key, &nonce, &ciphertext)?;
  let client_public_key_sha256 = sha256_hex(payload.client_public_key.as_bytes());

  Ok(DecryptedAuthPayload {
    payload,
    client_public_key_sha256,
  })
}

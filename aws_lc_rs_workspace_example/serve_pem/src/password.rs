use std::{env, sync::Arc};

use argon2::{
  Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version,
  password_hash::{PasswordHash, SaltString, rand_core::OsRng},
};
use zeroize::Zeroize;

use crate::{
  MIN_PASSWORD_PEPPER_BYTES, PASSWORD_HASH_ITERATIONS, PASSWORD_HASH_LENGTH,
  PASSWORD_HASH_MEMORY_COST_KIB, PASSWORD_HASH_PARALLELISM, PASSWORD_PEPPER_ENV, error::ApiError,
};

pub(crate) fn load_password_pepper() -> Result<Option<Vec<u8>>, String> {
  let Some(pepper) = env::var(PASSWORD_PEPPER_ENV).ok() else {
    return Ok(None);
  };

  if pepper.trim().is_empty() {
    return Ok(None);
  }

  if pepper.len() < MIN_PASSWORD_PEPPER_BYTES {
    return Err(format!(
      "{PASSWORD_PEPPER_ENV} must be at least {MIN_PASSWORD_PEPPER_BYTES} bytes when set"
    ));
  }

  Ok(Some(pepper.into_bytes()))
}

fn argon2_hasher(password_pepper: Option<&[u8]>) -> Result<Argon2<'_>, ApiError> {
  let params = Params::new(
    PASSWORD_HASH_MEMORY_COST_KIB,
    PASSWORD_HASH_ITERATIONS,
    PASSWORD_HASH_PARALLELISM,
    Some(PASSWORD_HASH_LENGTH),
  )
  .map_err(|_| {
    ApiError::internal(
      "password_hash_config_invalid",
      "password hash configuration is invalid",
    )
  })?;

  match password_pepper {
    Some(secret) => Argon2::new_with_secret(secret, Algorithm::Argon2id, Version::V0x13, params)
      .map_err(|_| {
        ApiError::internal(
          "password_hash_config_invalid",
          "password hash configuration is invalid",
        )
      }),
    None => Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params)),
  }
}

pub fn hash_password(password: &str, password_pepper: Option<&[u8]>) -> Result<String, ApiError> {
  let salt = SaltString::generate(&mut OsRng);
  let argon2 = argon2_hasher(password_pepper)?;

  argon2
    .hash_password(password.as_bytes(), &salt)
    .map(|password_hash| password_hash.to_string())
    .map_err(|_| ApiError::internal("password_hash_failed", "failed to hash password"))
}

pub(crate) async fn hash_password_for_storage(
  password: String,
  password_pepper: Option<Arc<Vec<u8>>>,
) -> Result<String, ApiError> {
  tokio::task::spawn_blocking(move || {
    let mut password = password;
    let hash_result = hash_password(&password, password_pepper.as_deref().map(Vec::as_slice));
    password.zeroize();
    hash_result
  })
  .await
  .map_err(|_| ApiError::internal("password_hash_task_failed", "password hashing task failed"))?
}

pub(crate) fn verify_password(
  password: &str,
  password_hash: &str,
  password_pepper: Option<&[u8]>,
) -> Result<bool, ApiError> {
  let parsed_hash = PasswordHash::new(password_hash)
    .map_err(|_| ApiError::internal("password_hash_invalid", "stored password hash is invalid"))?;
  let argon2 = argon2_hasher(password_pepper)?;

  Ok(
    argon2
      .verify_password(password.as_bytes(), &parsed_hash)
      .is_ok(),
  )
}

pub(crate) async fn verify_password_for_login(
  password: String,
  password_hash: String,
  password_pepper: Option<Arc<Vec<u8>>>,
) -> Result<bool, ApiError> {
  tokio::task::spawn_blocking(move || {
    let mut password = password;
    let verify_result = verify_password(
      &password,
      &password_hash,
      password_pepper.as_deref().map(Vec::as_slice),
    );
    password.zeroize();
    verify_result
  })
  .await
  .map_err(|_| {
    ApiError::internal(
      "password_verify_task_failed",
      "password verification task failed",
    )
  })?
}

use sqlx::{FromRow, PgPool};

use crate::error::ApiError;

fn map_insert_user_error(error: sqlx::Error) -> ApiError {
  if let sqlx::Error::Database(database_error) = &error {
    if database_error.code().as_deref() == Some("23505") {
      return ApiError::conflict(
        "client_public_key_exists",
        "a user with this client_public_key is already registered",
      );
    }
  }

  ApiError::internal(
    "database_insert_failed",
    "failed to store the registered user",
  )
}

pub(crate) async fn insert_registered_user(
  db_pool: &PgPool,
  client_public_key: &str,
  client_public_key_sha256: &str,
  password_hash: &str,
) -> Result<i64, ApiError> {
  sqlx::query_scalar::<_, i64>(
    r#"
    INSERT INTO users (client_public_key, client_public_key_sha256, password_hash)
    VALUES ($1, $2, $3)
    RETURNING id
    "#,
  )
  .bind(client_public_key)
  .bind(client_public_key_sha256)
  .bind(password_hash)
  .fetch_one(db_pool)
  .await
  .map_err(map_insert_user_error)
}

#[derive(Debug, FromRow)]
pub(crate) struct StoredUserCredentials {
  pub(crate) id: i64,
  pub(crate) client_public_key_sha256: String,
  pub(crate) password_hash: String,
}

pub(crate) async fn find_user_credentials(
  db_pool: &PgPool,
  client_public_key_sha256: &str,
) -> Result<Option<StoredUserCredentials>, ApiError> {
  sqlx::query_as::<_, StoredUserCredentials>(
    r#"
    SELECT id, client_public_key_sha256, password_hash
    FROM users
    WHERE client_public_key_sha256 = $1
    "#,
  )
  .bind(client_public_key_sha256)
  .fetch_optional(db_pool)
  .await
  .map_err(|_| ApiError::internal("database_query_failed", "failed to query user credentials"))
}

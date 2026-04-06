use std::fmt;

use axum::{
  Json,
  http::StatusCode,
  response::{IntoResponse, Response},
};
use serde_json::json;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug)]
pub struct AppError {
  pub status: StatusCode,
  pub message: String,
}

impl fmt::Display for AppError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.message)
  }
}

impl std::error::Error for AppError {}

impl AppError {
  pub fn bad_request(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::BAD_REQUEST,
      message: message.into(),
    }
  }

  pub fn unauthorized(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::UNAUTHORIZED,
      message: message.into(),
    }
  }

  pub fn conflict(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::CONFLICT,
      message: message.into(),
    }
  }

  pub fn internal(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::INTERNAL_SERVER_ERROR,
      message: message.into(),
    }
  }

  pub fn service_unavailable(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::SERVICE_UNAVAILABLE,
      message: message.into(),
    }
  }
}

impl IntoResponse for AppError {
  fn into_response(self) -> Response {
    let body = Json(json!({ "error": self.message }));
    (self.status, body).into_response()
  }
}

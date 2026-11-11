use axum::{
  Json,
  http::StatusCode,
  response::{IntoResponse, Response},
};
use serde::Serialize;

pub(crate) struct ApiError(pub StatusCode, pub &'static str);

#[derive(Serialize)]
struct ErrorBody {
  error: &'static str,
}

impl ApiError {
  pub(crate) fn bad_request(message: &'static str) -> Self {
    Self(StatusCode::BAD_REQUEST, message)
  }

  pub(crate) fn unauthorized() -> Self {
    Self(
      StatusCode::UNAUTHORIZED,
      "missing, invalid or expired credentials",
    )
  }

  pub(crate) fn forbidden(message: &'static str) -> Self {
    Self(StatusCode::FORBIDDEN, message)
  }

  pub(crate) fn not_found() -> Self {
    Self(StatusCode::NOT_FOUND, "session not found")
  }

  pub(crate) fn conflict(message: &'static str) -> Self {
    Self(StatusCode::CONFLICT, message)
  }

  pub(crate) fn internal() -> Self {
    Self(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
  }
}

impl IntoResponse for ApiError {
  fn into_response(self) -> Response {
    (self.0, Json(ErrorBody { error: self.1 })).into_response()
  }
}

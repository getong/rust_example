use axum::{
  Json,
  http::StatusCode,
  response::{IntoResponse, Response},
};

use crate::models::ErrorResponse;

#[derive(Debug, Clone, Copy)]
pub struct ApiError {
  pub(crate) status: StatusCode,
  pub(crate) code: &'static str,
  pub(crate) message: &'static str,
}

impl ApiError {
  pub(crate) const fn bad_request(code: &'static str, message: &'static str) -> Self {
    Self {
      status: StatusCode::BAD_REQUEST,
      code,
      message,
    }
  }

  pub(crate) const fn unsupported_media_type(code: &'static str, message: &'static str) -> Self {
    Self {
      status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
      code,
      message,
    }
  }

  pub(crate) const fn payload_too_large(code: &'static str, message: &'static str) -> Self {
    Self {
      status: StatusCode::PAYLOAD_TOO_LARGE,
      code,
      message,
    }
  }

  pub(crate) const fn conflict(code: &'static str, message: &'static str) -> Self {
    Self {
      status: StatusCode::CONFLICT,
      code,
      message,
    }
  }

  pub(crate) const fn unauthorized(code: &'static str, message: &'static str) -> Self {
    Self {
      status: StatusCode::UNAUTHORIZED,
      code,
      message,
    }
  }

  pub(crate) const fn internal(code: &'static str, message: &'static str) -> Self {
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

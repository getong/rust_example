use tuono_lib::{axum::http::StatusCode, Request};

// curl http://localhost:3000/api/health_check
#[tuono_lib::api(GET)]
pub async fn health_check(_req: Request) -> StatusCode {
  StatusCode::OK
}

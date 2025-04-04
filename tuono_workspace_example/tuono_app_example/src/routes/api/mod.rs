use tuono_lib::{Request, axum::http::StatusCode};

#[tuono_lib::api(GET)]
pub async fn health_check(_req: Request) -> StatusCode {
  StatusCode::OK
}

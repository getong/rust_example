use tuono_lib::{axum::http::StatusCode, Request};

// curl -X POST http://localhost:3000/api/post_check
#[tuono_lib::api(POST)]
pub async fn health_check(req: Request) -> StatusCode {
  println!("req is {:?}", req);
  StatusCode::OK
}

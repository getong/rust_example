use poem_openapi::{payload::Json, ApiResponse, OpenApi};

use super::{CalculatorRequest, CalculatorResponse, ServiceTags};

pub struct Subtractor;

#[derive(serde::Serialize, poem_openapi::Object)]
struct Error {
  message: String,
}

#[derive(ApiResponse)]
enum SubtractorResponse {
  #[oai(status = 200)]
  Ok(Json<CalculatorResponse>),
  #[oai(status = 400)]
  BadRequest(Json<Error>),
}

#[OpenApi(tag = "ServiceTags::Subtractor", prefix_path = "/sub")]
impl Subtractor {
  #[oai(path = "/", method = "post")]
  async fn index(&self, req: Json<CalculatorRequest>) -> SubtractorResponse {
    let res = req.a - req.b;
    if res < 0 {
      return SubtractorResponse::BadRequest(Json(Error {
        message: "Result is negative".to_string(),
      }));
    }
    SubtractorResponse::Ok(Json(CalculatorResponse { result: res }))
  }
}

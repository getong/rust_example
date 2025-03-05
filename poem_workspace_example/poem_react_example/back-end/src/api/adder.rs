use poem_openapi::{payload::Json, OpenApi};

use super::{CalculatorRequest, CalculatorResponse, ServiceTags};

pub struct Adder;

#[OpenApi(tag = "ServiceTags::Adder", prefix_path = "/add")]
impl Adder {
  #[oai(path = "/", method = "post")]
  async fn index(&self, req: Json<CalculatorRequest>) -> Json<CalculatorResponse> {
    Json(CalculatorResponse {
      result: req.a + req.b,
    })
  }
}

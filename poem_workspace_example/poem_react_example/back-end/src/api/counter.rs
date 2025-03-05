use poem_openapi::{payload::Json, OpenApi};
use tokio::sync::Mutex;

use super::{CalculatorResponse, ServiceTags};

pub struct Counter(pub Mutex<i32>);

#[OpenApi(tag = "ServiceTags::Counter", prefix_path = "/counter")]
impl Counter {
  #[oai(path = "/", method = "get")]
  async fn index(&self) -> Json<CalculatorResponse> {
    let mut counter = self.0.lock().await;
    *counter += 1;
    Json(CalculatorResponse { result: *counter })
  }
}

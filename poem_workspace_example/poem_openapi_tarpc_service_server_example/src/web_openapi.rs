use poem_openapi::{param::Query, payload::PlainText, OpenApi};

use crate::common::Api;

#[OpenApi]
impl Api {
  #[oai(path = "/hello", method = "get")]
  pub async fn index(&self, name: Query<Option<String>>) -> PlainText<String> {
    let recv_name = match name.0 {
      Some(name) => name,
      None => "unknown!".to_string(),
    };
    PlainText(format!(
      "hello, {}, the current num is {:?}!\n",
      recv_name,
      self.num.lock().await
    ))
  }
}

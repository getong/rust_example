use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GreeRequest {
  pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GreetResponse {
  pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnotherMessage {
  pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FourthMessage {
  pub message: String,
}

// Enum to wrap all message types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")] // Optional: Use tags to differentiate types in JSON
pub enum Message {
  GreeRequest(GreeRequest),
  GreetResponse(GreetResponse),
  AnotherMessage(AnotherMessage),
  FourthMessage(FourthMessage),
}

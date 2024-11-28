use serde::{Deserialize, Serialize};
use serde_json::{self, Error as SerdeJsonError};

#[derive(Debug, Serialize, Deserialize)]
pub struct GreeRequest {
  pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GreetResponse {
  pub message: String,
}

// New message types can be added as needed
#[derive(Debug, Serialize, Deserialize)]
pub struct AnotherMessage {
  pub info: String,
}

// Enum to wrap all message types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")] // Optional: Use tags to differentiate types in JSON
pub enum Message {
  GreeRequest(GreeRequest),
  GreetResponse(GreetResponse),
  AnotherMessage(AnotherMessage),
}

impl Message {
  /// Convert to binary JSON (`Vec<u8>`)
  pub fn to_binary(&self) -> Result<Vec<u8>, SerdeJsonError> {
    serde_json::to_vec(self)
  }

  /// Create from binary JSON (`Vec<u8>`)
  pub fn from_binary(data: &[u8]) -> Result<Self, SerdeJsonError> {
    serde_json::from_slice(data)
  }
}

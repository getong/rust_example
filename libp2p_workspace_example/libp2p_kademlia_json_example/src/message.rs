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

impl GreeRequest {
  /// Convert to binary JSON (`Vec<u8>`)
  pub fn to_binary(&self) -> Result<Vec<u8>, SerdeJsonError> {
    serde_json::to_vec(self)
  }

  /// Create from binary JSON (`Vec<u8>`)
  pub fn from_binary(data: &[u8]) -> Result<Self, SerdeJsonError> {
    serde_json::from_slice(data)
  }
}

impl GreetResponse {
  /// Convert to binary JSON (`Vec<u8>`)
  pub fn to_binary(&self) -> Result<Vec<u8>, SerdeJsonError> {
    serde_json::to_vec(self)
  }

  /// Create from binary JSON (`Vec<u8>`)
  pub fn from_binary(data: &[u8]) -> Result<Self, SerdeJsonError> {
    serde_json::from_slice(data)
  }
}

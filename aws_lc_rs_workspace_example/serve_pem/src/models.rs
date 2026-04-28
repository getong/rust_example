use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zeroize::Zeroize;

#[derive(Serialize, Deserialize)]
pub(crate) struct PublicKeyResponse {
  pub(crate) transport: &'static str,
  pub(crate) key_encryption_algorithm: &'static str,
  pub(crate) content_encryption_algorithm: &'static str,
  pub(crate) key_format: &'static str,
  pub(crate) public_key_pem: String,
  pub(crate) public_key_der_base64: String,
  pub(crate) sha256_hash: String,
  pub(crate) wrapped_key_bytes: usize,
  pub(crate) nonce_bytes: usize,
  pub(crate) max_wrapped_key_plaintext_bytes: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegisterRequest {
  pub(crate) wrapped_key_base64: String,
  pub(crate) nonce_base64: String,
  pub(crate) ciphertext_base64: String,
}

#[derive(Debug, Deserialize, Serialize, Zeroize)]
#[zeroize(drop)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegistrationPayload {
  pub(crate) client_public_key: String,
  pub(crate) password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RegisterResponse {
  pub(crate) status: &'static str,
  pub(crate) user_id: i64,
  pub(crate) client_public_key_sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct LoginResponse {
  pub(crate) status: &'static str,
  pub(crate) user_id: i64,
  pub(crate) client_public_key_sha256: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ErrorResponse {
  pub(crate) code: &'static str,
  pub(crate) error: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClientWsEnvelope {
  pub(crate) event: String,
  pub(crate) payload: ClientChatPayload,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClientChatPayload {
  pub(crate) user: Option<String>,
  pub(crate) msg: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ServerWsEnvelope {
  pub(crate) event: &'static str,
  pub(crate) payload: Value,
}

impl ServerWsEnvelope {
  pub(crate) fn welcome(room: &str, who: SocketAddr, member_count: usize) -> Self {
    Self {
      event: "welcome",
      payload: json!({
        "room": room,
        "connection": who.to_string(),
        "member_count": member_count,
      }),
    }
  }

  pub(crate) fn presence(
    action: &'static str,
    room: &str,
    member_count: usize,
    who: Option<String>,
  ) -> Self {
    Self {
      event: "presence",
      payload: json!({
        "action": action,
        "room": room,
        "member_count": member_count,
        "connection": who,
      }),
    }
  }

  pub(crate) fn chat_message(room: &str, who: SocketAddr, user: &str, msg: &str) -> Self {
    Self {
      event: "chat_message",
      payload: json!({
        "room": room,
        "user": user,
        "msg": msg,
        "from": who.to_string(),
      }),
    }
  }

  pub(crate) fn error(message: &str) -> Self {
    Self {
      event: "error",
      payload: json!({
        "message": message,
      }),
    }
  }
}

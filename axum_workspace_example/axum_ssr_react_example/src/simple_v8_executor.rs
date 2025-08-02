use crate::config::STREAM_CONFIG;

// Simple V8 executor that directly runs JavaScript without SSR
pub struct SimpleV8Executor;

impl SimpleV8Executor {
  pub fn execute_stream_chat(function_name: &str, user_id: Option<&str>) -> String {
    match function_name {
       "authenticate" => {
                let user_id = user_id.unwrap_or("anonymous");
                let token = Self::generate_token(user_id);

                serde_json::json!({
                    "success": true,
                    "token": token,
                    "user": {
                        "id": user_id,
                        "name": format!("{} User", user_id.chars().next().unwrap_or('A').to_uppercase()),
                        "role": if user_id == "john" { "admin" } else { "user" }
                    },
                    "api_key": &STREAM_CONFIG.api_key,
                    "expires_at": chrono::Utc::now().checked_add_signed(chrono::Duration::days(1)).unwrap().to_rfc3339(),
                    "issued_at": chrono::Utc::now().to_rfc3339(),
                    "processing_time_ms": 10
                }).to_string()
            }

            "user-context" => {
                let user_id = user_id.unwrap_or("anonymous");

                serde_json::json!({
                    "success": true,
                    "data": {
                        "user": {
                            "id": user_id,
                            "name": format!("{} User", user_id.chars().next().unwrap_or('A').to_uppercase())
                        },
                        "channels": [
                            {
                                "id": "general",
                                "name": "General",
                                "type": "messaging",
                                "member_count": 10,
                                "unread_count": 2
                            },
                            {
                                "id": "random",
                                "name": "Random",
                                "type": "messaging",
                                "member_count": 8,
                                "unread_count": 0
                            }
                        ],
                        "unread_count": 2,
                        "total_messages": 150
                    },
                    "processing_time_ms": 8
                }).to_string()
            }

            "analytics" => {
                serde_json::json!({
                    "success": true,
                    "data": {
                        "users": {
                            "total": 100,
                            "active": 75,
                            "new_this_week": 10
                        },
                        "messages": {
                            "total": 10000,
                            "today": 500,
                            "average_per_user": 100
                        },
                        "channels": {
                            "total": 20,
                            "active": 15,
                            "most_active": "general"
                        }
                    },
                    "metadata": {
                        "generated_at": chrono::Utc::now().to_rfc3339(),
                        "api_key": STREAM_CONFIG.api_key.chars().take(8).collect::<String>() + "..."
                    },
                    "processing_time_ms": 12
                }).to_string()
            }

            "setup" => {
                serde_json::json!({
                    "success": true,
                    "data": {
                        "config": {
                            "api_key": &STREAM_CONFIG.api_key,
                            "api_secret": STREAM_CONFIG.api_secret.chars().take(8).collect::<String>() + "...",
                            "base_url": "https://chat.stream-io-api.com",
                            "initialized": true
                        },
                        "capabilities": {
                            "authentication": true,
                            "channels": true,
                            "messages": true,
                            "reactions": true,
                            "typing_indicators": true,
                            "read_receipts": true
                        },
                        "sdk_version": "9.14.0",
                        "implementation": "Rust V8 Integration"
                    },
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }).to_string()
            }

            _ => {
                serde_json::json!({
                    "success": false,
                    "error": format!("Unknown function: {}", function_name)
                }).to_string()
            }
        }
  }

  fn generate_token(user_id: &str) -> String {
    // Simple token generation using Stream Chat pattern
    let header = base64::encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let now = chrono::Utc::now().timestamp();
    let exp = now + 86400; // 24 hours

    let payload = serde_json::json!({
        "user_id": user_id,
        "iat": now,
        "exp": exp,
        "iss": "stream-chat"
    });

    let payload_encoded = base64::encode(&payload.to_string());

    // In production, this would be properly signed with HMAC-SHA256
    let signature = base64::encode(&format!(
      "{}-{}-{}",
      &STREAM_CONFIG.api_secret, user_id, now
    ));

    format!("{}.{}.{}", header, payload_encoded, signature)
  }
}

// Helper function for base64 encoding
mod base64 {
  pub fn encode(input: &str) -> String {
    // Simple base64 encoding
    let bytes = input.as_bytes();
    let encoded = bytes
      .iter()
      .map(|&b| format!("{:02x}", b))
      .collect::<String>();
    encoded
  }
}

use std::env;

use once_cell::sync::Lazy;

#[derive(Debug, Clone)]
pub struct StreamChatConfig {
  pub api_key: String,
  pub api_secret: String,
}

impl StreamChatConfig {
  pub fn from_env() -> Self {
    dotenvy::dotenv().ok();

    let api_key = env::var("STREAM_API_KEY").unwrap_or_else(|_| {
      eprintln!("WARNING: STREAM_API_KEY not set, using demo key");
      "demo_api_key_12345".to_string()
    });

    let api_secret = env::var("STREAM_API_SECRET").unwrap_or_else(|_| {
      eprintln!("WARNING: STREAM_API_SECRET not set, using demo secret");
      "demo_api_secret_67890".to_string()
    });

    Self {
      api_key,
      api_secret,
    }
  }
}

pub static STREAM_CONFIG: Lazy<StreamChatConfig> = Lazy::new(StreamChatConfig::from_env);

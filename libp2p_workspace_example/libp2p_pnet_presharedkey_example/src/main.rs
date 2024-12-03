use std::{error::Error, str::FromStr};

use base64::{Engine, engine::general_purpose::STANDARD};
use libp2p::pnet::PreSharedKey;

// $ openssl rand -base64 32
// G31AI1XEwAtS26ocfx3zfXxl1xbxQNXan9VeNO6YztQ=

/// Helper function to decode a base64-encoded key
fn decode_base64_key(base64_key: &str) -> Result<[u8; 32], Box<dyn Error>> {
  let bytes = STANDARD.decode(base64_key)?;
  let key: [u8; 32] = bytes.try_into().expect("Decoded key must be 32 bytes long");
  Ok(key)
}

/// Helper function to create a PreSharedKey and print its details
fn handle_psk_operations(psk: &PreSharedKey) {
  println!("Fingerprint: {}", psk.fingerprint());
  println!("PSK String: {}", psk.to_string());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // Base64 encoded key
  const BASE64_KEY: &str = "G31AI1XEwAtS26ocfx3zfXxl1xbxQNXan9VeNO6YztQ=";

  // Decode the base64 key
  let key = decode_base64_key(BASE64_KEY)?;

  // Create a PreSharedKey and print its details
  let psk = PreSharedKey::new(key);
  handle_psk_operations(&psk);

  // Example PSK string
  const PSK_STRING: &str = "/key/swarm/psk/1.0.0/
/base16/
1b7d402355c4c00b52dbaa1c7f1df37d7c65d716f140d5da9fd55e34ee98ced4";

  // Parse the PSK string and print its fingerprint
  let parsed_psk = PreSharedKey::from_str(PSK_STRING).expect("Failed to parse PSK string");
  println!("Parsed PSK Fingerprint: {}", parsed_psk.fingerprint());

  Ok(())
}

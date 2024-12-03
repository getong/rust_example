use std::error::Error;

use base64::{Engine, engine::general_purpose::STANDARD};
use libp2p::pnet::PreSharedKey;

// openssl rand -base64 32
// G31AI1XEwAtS26ocfx3zfXxl1xbxQNXan9VeNO6YztQ=

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // Base64 encoded key
  let base64_key = "G31AI1XEwAtS26ocfx3zfXxl1xbxQNXan9VeNO6YztQ=";

  // Decode the base64 key
  let bytes = STANDARD.decode(base64_key)?;

  // Ensure the decoded bytes are exactly 32 bytes long
  let key: [u8; 32] = bytes.try_into().expect("Key must be 32 bytes long");

  // Create a pre-shared key
  let psk = PreSharedKey::new(key);

  // Generate the fingerprint
  let fingerprint = psk.fingerprint();

  // Print the fingerprint
  println!("Fingerprint: {}", fingerprint);

  Ok(())
}

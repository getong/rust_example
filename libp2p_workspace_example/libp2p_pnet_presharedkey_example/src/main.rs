use std::{error::Error, str::FromStr};

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

  let psk_string = psk.to_string();
  println!("psk_string: {}", psk_string);

  let psk_string = "/key/swarm/psk/1.0.0/
/base16/
1b7d402355c4c00b52dbaa1c7f1df37d7c65d716f140d5da9fd55e34ee98ced4";

  let psk = PreSharedKey::from_str(psk_string).expect("not work");
  println!("psk finger print: {}", psk.fingerprint());

  Ok(())
}

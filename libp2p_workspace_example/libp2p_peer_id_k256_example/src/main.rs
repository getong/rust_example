use k256::{ecdsa::SigningKey, elliptic_curve::rand_core::OsRng};
use libp2p::{
  PeerId,
  identity::{self, Keypair},
};

// Function to generate a secp256k1 private key and return the trimmed private key as a hex string
fn generate_secp256k1_key() -> Result<String, Box<dyn std::error::Error>> {
  // Generate a new signing key using the secp256k1 curve
  let signing_key = SigningKey::random(&mut OsRng);

  // Get the raw bytes of the private key
  let private_key_bytes = signing_key.to_bytes();

  // Convert to hexadecimal and trim to 64 characters
  let private_key_hex = hex::encode(private_key_bytes);
  let private_key_trimmed = private_key_hex[.. 64].to_string(); // Extract first 64 characters

  Ok(private_key_trimmed)
}

fn is_valid_key(key: &str) -> bool {
  // Decode the hex string into bytes
  if let Ok(private_key_bytes) = hex::decode(key) {
    // Check if the length of the private key is valid
    if private_key_bytes.len() != 32 {
      return false;
    }

    if let Ok(secret_key) = identity::secp256k1::SecretKey::try_from_bytes(private_key_bytes) {
      let libp2p_keypair: Keypair = identity::secp256k1::Keypair::from(secret_key).into();
      let peer_id = PeerId::from(libp2p_keypair.public());
      println!("key : {}\npeer_id : {}", key, peer_id.to_base58());
      true
    } else {
      false
    }
  } else {
    false
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut keys = Vec::new();

  // Generate and process six keys
  for _ in 0 .. 6 {
    if let Ok(key) = generate_secp256k1_key() {
      if is_valid_key(&key) {
        keys.push(key);
      }
    }
  }

  // Print all trimmed keys
  for (i, key) in keys.iter().enumerate() {
    println!("Key {}: {}", i + 1, key);
  }

  Ok(())
}

use libp2p::{PeerId, identity};

fn main() {
  // Normally, you would use a valid secp256k1 public key
  let public_key_bytes =
    hex::decode("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
      .expect("Invalid hex string");

  // Create a libp2p secp256k1 PublicKey from bytes
  let secp256k1_public_key = identity::secp256k1::PublicKey::try_from_bytes(&public_key_bytes)
    .expect("Failed to parse libp2p secp256k1 public key");

  let hex_string = hex::encode(secp256k1_public_key.to_bytes());
  println!("hex_string: {}", hex_string);

  // Convert secp256k1 public key to libp2p public key
  let libp2p_public_key = secp256k1_public_key.into();

  // Create a PeerId from the public key
  let peer_id = PeerId::from_public_key(&libp2p_public_key);

  println!("Generated PeerId: {}", peer_id);
}

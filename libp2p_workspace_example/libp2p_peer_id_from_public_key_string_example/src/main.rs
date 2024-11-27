use ethers::core::utils::parse_checksummed;
use libp2p::{PeerId, identity};

fn main() {
  // Example hex string with `0x` prefix and mixed case
  let hex_string = "0x30597420A16Dbf72A3e2A4309d00c436600d3AA7";

  // Strip the `0x` prefix if it exists and downcase the string
  let public_key_bytes = parse_checksummed(hex_string, None).expect("not ethers public key");

  // Create a libp2p secp256k1 PublicKey from bytes
  let secp256k1_public_key =
    identity::secp256k1::PublicKey::try_from_bytes(&public_key_bytes.as_bytes())
      .expect("Failed to parse libp2p secp256k1 public key");

  // Convert secp256k1 public key to libp2p public key
  let libp2p_public_key = secp256k1_public_key.into();

  // Create a PeerId from the public key
  let peer_id = PeerId::from_public_key(&libp2p_public_key);

  println!("Generated PeerId: {}", peer_id);
}

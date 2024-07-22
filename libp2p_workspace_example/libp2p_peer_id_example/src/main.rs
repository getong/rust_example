use libp2p::{identity, PeerId};

fn main() {
  generate_ed25519();

  generate_secp256k1();

  generate_ecdsa();
}

fn generate_ed25519() {
  // Generate a keypair
  let keypair = identity::Keypair::generate_ed25519();

  // Derive the PeerId from the keypair
  let peer_id = PeerId::from(keypair.public());

  println!("Generated ed25519 PeerId: {:?}", peer_id);

  let private_key = keypair.to_protobuf_encoding().unwrap();
  let hex_string = hex::encode(private_key);

  println!("ed25519 : {}", hex_string);
}

fn generate_secp256k1() {
  // Generate a keypair
  let keypair = identity::Keypair::generate_secp256k1();

  // Derive the PeerId from the keypair
  let peer_id = PeerId::from(keypair.public());

  println!("Generated secp256k1 PeerId: {:?}", peer_id);

  let private_key = keypair.to_protobuf_encoding().unwrap();
  let hex_string = hex::encode(private_key);

  println!("secp256k1 : {}", hex_string);
}

fn generate_ecdsa() {
  // Generate a keypair
  let keypair = identity::Keypair::generate_ecdsa();

  // Derive the PeerId from the keypair
  let peer_id = PeerId::from(keypair.public());

  println!("Generated ecdsa PeerId: {:?}", peer_id);

  let private_key = keypair.to_protobuf_encoding().unwrap();
  let hex_string = hex::encode(private_key);

  println!("ecdsa : {}", hex_string);
}

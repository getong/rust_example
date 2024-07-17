use libp2p::{identity, PeerId};

fn main() {
  // Generate a keypair
  let keypair = identity::Keypair::generate_ed25519();

  // Derive the PeerId from the keypair
  let peer_id = PeerId::from(keypair.public());

  println!("Generated PeerId: {:?}", peer_id);

  let private_key = keypair.to_protobuf_encoding().unwrap();
  let hex_string = hex::encode(private_key);

  println!("{}", hex_string);
}

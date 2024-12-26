use bs58::decode;
use libp2p::PeerId;

fn main() {
  let peer_id = PeerId::random();
  println!("peer_id is {}", peer_id);

  // Convert PeerId to base58 string
  let base58_str = peer_id.to_base58();
  println!("base58_str: {}", base58_str);

  // Convert PeerId to hex string
  let hex_str = hex::encode(peer_id.to_bytes());
  println!("hex_str: {}", hex_str);

  // Revert base58 string back to PeerId
  let peer_id_from_base58 = PeerId::from_bytes(
    &decode(base58_str)
      .into_vec()
      .expect("Invalid base58 string"),
  )
  .expect("Failed to parse PeerId from base58");
  println!("peer_id_from_base58: {}", peer_id_from_base58);

  assert_eq!(peer_id, peer_id_from_base58);

  // Revert hex string back to PeerId
  let peer_id_from_hex = PeerId::from_bytes(&hex::decode(hex_str).expect("Invalid hex string"))
    .expect("Failed to parse PeerId from hex");
  println!("peer_id_from_hex: {}", peer_id_from_hex);
  assert_eq!(peer_id, peer_id_from_hex);
}

// openssl ecparam -name secp256k1 -genkey -noout -out private_key.pem
//
// openssl ec -in private_key.pem -pubout -outform DER | tail -c 65 | xxd -p -c 65 > public_key.txt
//
use std::{error::Error, fs};

use ethers::{
  prelude::Address,
  utils::{keccak256, to_checksum},
};
use libp2p::{
  PeerId,
  identity::{self, Keypair, secp256k1::PublicKey as Secp256k1PublicKey},
};
use secp256k1::PublicKey;

pub fn pub_key_to_eth_address(pub_key: &PublicKey) -> Result<String, Box<dyn Error>> {
  // Serialize the public key in uncompressed format (65 bytes)
  let pub_key_bytes = pub_key.serialize_uncompressed();

  // Calculate the Ethereum address by hashing the X and Y coordinates (skip the first byte)
  let hash = keccak256(&pub_key_bytes[1 ..]); // Skip the 0x04 prefix
  let address = Address::from_slice(&hash[12 ..]);

  Ok(to_checksum(&address, None).to_lowercase())
}

pub fn secpe256k1_publickey_to_eth_address(
  pub_key: &Secp256k1PublicKey,
) -> Result<String, Box<dyn Error>> {
  let pub_key_bytes = pub_key.to_bytes_uncompressed();

  let hash = keccak256(&pub_key_bytes[1 ..]); // Skip the 0x04 prefix
  let address = Address::from_slice(&hash[12 ..]);

  Ok(to_checksum(&address, None).to_lowercase())
}

pub fn make_fake_libp2p_keypair() -> Keypair {
  let private_key_bytes =
    hex::decode("0000000000000000000000000000000000000000000000000000000000000001").unwrap();
  let secret_key = identity::secp256k1::SecretKey::try_from_bytes(private_key_bytes).unwrap();
  identity::secp256k1::Keypair::from(secret_key).into()
}

fn main() -> Result<(), Box<dyn Error>> {
  let file_path = "public_key.txt";

  let public_key_str = fs::read_to_string(file_path)?.trim().to_string();

  if public_key_str.is_empty() {
    return Err(format!("Public key is empty in file: {}", file_path).into());
  }
  let public_key_str = if public_key_str.starts_with("0x") {
    &public_key_str[2 ..]
  } else {
    &public_key_str
  };

  let public_key_bytes =
    hex::decode(public_key_str).map_err(|_| "Invalid hex format in public_key.txt")?;

  println!("Decoded Public Key Bytes: {:?}", public_key_bytes);

  if public_key_bytes.len() != 65 {
    return Err(
      format!(
        "Public key must be exactly 65 bytes in uncompressed format. Got: {} bytes",
        public_key_bytes.len()
      )
      .into(),
    );
  }

  if public_key_bytes[0] != 0x04 {
    return Err("Public key does not start with the uncompressed prefix (0x04).".into());
  }

  let secp_key = PublicKey::from_slice(&public_key_bytes)
    .map_err(|e| format!("Failed to parse secp256k1 public key: {}", e))?;
  println!("Valid secp256k1 Public Key: {:?}", secp_key);

  // Example usage of get_eth_addr_from_peer
  let eth_address = pub_key_to_eth_address(&secp_key)?;
  println!(
    "Ethereum Address: {}, it is the same with others",
    eth_address
  );

  // Test compressed format
  let compressed_pub_key = secp_key.serialize(); // 33 bytes
  println!("Compressed Public Key: {:?}", compressed_pub_key);

  let libp2p_pub_key = Secp256k1PublicKey::try_from_bytes(&compressed_pub_key)
    .or_else(|_| {
      // If compressed fails, try uncompressed format
      let uncompressed_pub_key = secp_key.serialize_uncompressed(); // 65 bytes
      println!("Uncompressed Public Key: {:?}", uncompressed_pub_key);
      Secp256k1PublicKey::try_from_bytes(&uncompressed_pub_key)
    })
    .map_err(|e| format!("Failed to parse libp2p secp256k1 public key: {}", e))?;

  let libp2p_eth_address = secpe256k1_publickey_to_eth_address(&libp2p_pub_key)?;
  println!(
    "libp2p Ethereum Address: {}, it is the same with others",
    libp2p_eth_address
  );

  let peer_id = PeerId::from_public_key(&libp2p_pub_key.into());
  println!("Generated PeerId: {}", peer_id);

  let fake_keypair = make_fake_libp2p_keypair();

  let libp2p_eth_address =
    secpe256k1_publickey_to_eth_address(&fake_keypair.public().try_into_secp256k1().unwrap())?;
  println!(
    "libp2p fake Ethereum Address: {}, it is the same with others",
    libp2p_eth_address
  );
  let fake_peer_id = PeerId::from_public_key(&fake_keypair.public());
  println!("Generated fake PeerId: {}", fake_peer_id);

  Ok(())
}

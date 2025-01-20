// openssl ecparam -name secp256k1 -genkey -noout -out private_key.pem
//
// openssl ec -in private_key.pem -pubout -outform DER | tail -c 65 | xxd -p -c 65 > public_key.txt
//

use std::{error::Error, fs};

use alloy::primitives::{Address, keccak256};
use libp2p::identity::{PublicKey as Libp2pPublicKey, secp256k1::PublicKey as Secp256k1PublicKey};
use secp256k1::PublicKey;

pub fn secpe256k1_publickey_to_eth_address(
  pub_key: &Secp256k1PublicKey,
) -> Result<String, Box<dyn Error>> {
  let pub_key_bytes = pub_key.to_bytes_uncompressed();

  let hash = keccak256(&pub_key_bytes[1 ..]); // Skip the 0x04 prefix
  let address = Address::from_slice(&hash[12 ..]);

  Ok(address.to_checksum(None).to_lowercase())
}

pub fn libp2p_publickey_to_eth_address(
  pub_key: &Libp2pPublicKey,
) -> Result<String, Box<dyn Error>> {
  let secp256k1_key = pub_key.clone().try_into_secp256k1().unwrap();
  let pub_key_bytes = secp256k1_key.to_bytes_uncompressed();

  let hash = keccak256(&pub_key_bytes[1 ..]); // Skip the 0x04 prefix
  let address = Address::from_slice(&hash[12 ..]);

  Ok(address.to_checksum(None).to_lowercase())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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

  let compressed_pub_key = secp_key.serialize(); // 33 bytes

  let libp2p_secp256k1_pubkey = Secp256k1PublicKey::try_from_bytes(&compressed_pub_key)
    .or_else(|_| {
      // If compressed fails, try uncompressed format
      let uncompressed_pub_key = secp_key.serialize_uncompressed(); // 65 bytes
      println!("Uncompressed Public Key: {:?}", uncompressed_pub_key);
      Secp256k1PublicKey::try_from_bytes(&uncompressed_pub_key)
    })
    .map_err(|e| format!("Failed to parse libp2p secp256k1 public key: {}", e))?;

  let libp2p_eth_address = secpe256k1_publickey_to_eth_address(&libp2p_secp256k1_pubkey)?;
  println!(
    "libp2p Ethereum Address: {}, it is the same with others",
    libp2p_eth_address
  );

  let libp2p_public_key = libp2p_secp256k1_pubkey.into();

  let libp2p_eth_address = libp2p_publickey_to_eth_address(&libp2p_public_key)?;
  println!(
    "libp2p Ethereum Address: {}, it is the same with others",
    libp2p_eth_address
  );

  Ok(())
}

use ethers_core::types::H256;

fn ipfs_hash_to_h256(ipfs_hash: &str) -> Result<H256, Box<dyn std::error::Error>> {
  // Decode base58 IPFS hash
  let decoded_bytes = bs58::decode(ipfs_hash).into_vec()?;

  // IPFS hash is a multihash: [prefix (2 bytes)] + [digest (32 bytes)]
  // Check length
  if decoded_bytes.len() != 34 {
    return Err("Invalid IPFS hash length".into());
  }

  // Extract the 32-byte digest part (skip the first 2 bytes)
  let digest_bytes = &decoded_bytes[2 ..];

  // Convert to H256 (32-byte hash)
  let h256 = H256::from_slice(digest_bytes);

  Ok(h256)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let ipfs_hash = "QmQbSumWfjgEfrPGsnNGAhq7SyXDjxKQCXLSjRiPpgFktd";
  let h256_hash = ipfs_hash_to_h256(ipfs_hash)?;

  println!("H256: {:?}", h256_hash);
  println!("Hex: {:?}", h256_hash);

  // Output in hexadecimal string (with 0x prefix)
  let hex_str = format!("{:#x}", h256_hash);
  println!("Hex string: {}", hex_str);

  Ok(())
}

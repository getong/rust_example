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
  Ok(H256::from_slice(digest_bytes))
}

// Converts H256 back to IPFS Base58 hash
fn h256_to_ipfs_hash(hash: &H256) -> String {
  // Multihash format:
  // First byte: hash function (0x12 = sha2-256)
  // Second byte: digest length (0x20 = 32 bytes)
  let mut multihash_bytes = Vec::with_capacity(34);
  multihash_bytes.push(0x12);
  multihash_bytes.push(0x20);
  multihash_bytes.extend_from_slice(hash.as_bytes());

  bs58::encode(multihash_bytes).into_string()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let original_ipfs_hash = "QmQbSumWfjgEfrPGsnNGAhq7SyXDjxKQCXLSjRiPpgFktd";
  println!("Original IPFS hash: {}", original_ipfs_hash);

  // Convert IPFS hash to H256
  let h256_hash = ipfs_hash_to_h256(original_ipfs_hash)?;
  println!("Converted to H256: {:?}", h256_hash);

  // Convert H256 back to IPFS hash
  let recovered_ipfs_hash = h256_to_ipfs_hash(&h256_hash);
  println!("Recovered IPFS hash: {}", recovered_ipfs_hash);

  assert_eq!(original_ipfs_hash, recovered_ipfs_hash);
  println!("✅ Hash conversion verified successfully!");

  Ok(())
}

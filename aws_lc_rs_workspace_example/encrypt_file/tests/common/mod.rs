use aws_lc_rs::digest::{SHA512, digest};

// Independent serial implementation of the documented v3 encoding; production uses Rayon
// and incremental Contexts. This also permits testing a valid signature over invalid AEAD data.
pub fn v3_transcript(header: &[u8], ciphertext: &[u8]) -> Vec<u8> {
  let mut root = b"encrypt_file/v3/SHA-512-TREE-1M/root\0".to_vec();
  root.extend_from_slice(&(ciphertext.len() as u64).to_le_bytes());
  root.extend_from_slice(&1048576u64.to_le_bytes());
  root.extend_from_slice(&(ciphertext.len().div_ceil(1048576) as u64).to_le_bytes());
  for (index, chunk) in ciphertext.chunks(1048576).enumerate() {
    let mut leaf = b"encrypt_file/v3/SHA-512-TREE-1M/leaf\0".to_vec();
    leaf.extend_from_slice(&(index as u64).to_le_bytes());
    leaf.extend_from_slice(&(chunk.len() as u64).to_le_bytes());
    leaf.extend_from_slice(chunk);
    root.extend_from_slice(digest(&SHA512, &leaf).as_ref());
  }
  let mut message = b"encrypt_file/v3/signature/ML-DSA-87/SHA-512-TREE-1M\0".to_vec();
  message.extend_from_slice(header);
  message.extend_from_slice(digest(&SHA512, &root).as_ref());
  message
}

use aes_gcm::{
  Aes256Gcm,
  aead::{Aead, Error as AeadError, KeyInit},
};
use anyhow::{Result, anyhow};
use digest::{Digest, generic_array::GenericArray};
use hex;

pub fn decrypt(secret_key: &str, ct: &str) -> Result<String> {
  let ct = if ct.starts_with("0x") { &ct[2 ..] } else { ct };
  let ct_bytes = hex::decode(ct).map_err(|e| anyhow!("Failed to decode hex: {}", e))?;
  let ct_len = ct_bytes.len();
  if ct_len < 28 {
    return Err(anyhow!("InvalidEncrypt(1042)"));
  }

  let iv = &ct_bytes[ct_len - 12 ..];
  let content = &ct_bytes[.. ct_len - 12];
  let nonce = GenericArray::from_slice(iv);

  let mut hasher = sha2::Sha256::new();
  hasher.update(secret_key.as_bytes());
  let gcm = Aes256Gcm::new_from_slice(hasher.finalize().as_slice())
    .map_err(|e| anyhow!("Failed to create AES-GCM cipher: {:?}", e))?;

  let ptext = gcm
    .decrypt(nonce, content)
    .map_err(|e: AeadError| anyhow!("Decryption failed: {:?}", e))?;

  Ok(String::from_utf8(ptext).map_err(|e| anyhow!("Invalid UTF-8 in decrypted text: {}", e))?)
}

fn main() {
  match decrypt("command-line-key", "0x1234567890abcdef") {
    Ok(sk) => println!("Decrypted: {}", sk[2 ..].to_string()),
    Err(e) => println!("Error: {:?}", e),
  };
}

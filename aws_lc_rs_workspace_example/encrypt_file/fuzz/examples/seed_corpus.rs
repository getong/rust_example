use std::{fs, path::PathBuf};

use aws_lc_rs::signature::{ML_DSA_87_SIGNING, PqdsaKeyPair};
use encrypt_file::{KEY_HEADER_LEN, PRIVATE_KEY_LEN, TAG_LEN, encrypt_bytes};
use zeroize::Zeroizing;
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus/decrypt");
  fs::create_dir_all(&corpus)?;
  fs::write(
    corpus.join("v1"),
    include_bytes!("../../tests/fixtures/v1.encrypted"),
  )?;
  fs::write(
    corpus.join("v2-fixed"),
    include_bytes!("../../tests/fixtures/v2.encrypted"),
  )?;
  let signer = PqdsaKeyPair::from_seed(&ML_DSA_87_SIGNING, &[23; 32])?;
  let signed = encrypt_bytes(
    Zeroizing::new(b"fuzz seed".to_vec()),
    include_bytes!("../../tests/fixtures/v1.public"),
    &signer,
  )?;
  fs::write(corpus.join("v3"), signed.to_bytes())?;
  let mut key = vec![0u8; KEY_HEADER_LEN + PRIVATE_KEY_LEN + TAG_LEN];
  key[.. 11].copy_from_slice(b"ALCFKEY\0\x02\x01\x01");
  for (offset, value) in [(11, 65536u32), (15, 3), (19, 1)] {
    key[offset .. offset + 4].copy_from_slice(&value.to_le_bytes());
  }
  fs::write(corpus.join("key-header"), key)?;
  Ok(())
}

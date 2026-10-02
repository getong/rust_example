mod common;

use std::{env, fs, path::Path};

use aws_lc_rs::{
  aead::{Aad, NONCE_LEN, Nonce},
  kem::{EncapsulationKey, ML_KEM_1024},
  rand::{SecureRandom, SystemRandom},
};
use common::{HEADER_LEN, KEM_END, PREFIX, Result, SALT_END, derive_key, read_key, write_new};

fn main() -> Result<()> {
  let args: Vec<_> = env::args_os().skip(1).collect();
  if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
    println!("用法：encrypt <公钥文件> <输入文件> <加密文件>");
    return Ok(());
  }
  if args.len() != 3 {
    return Err("用法：encrypt <公钥文件> <输入文件> <加密文件>".into());
  }
  encrypt_file(
    Path::new(&args[0]),
    Path::new(&args[1]),
    Path::new(&args[2]),
  )?;
  println!("加密成功：{}", Path::new(&args[2]).display());
  Ok(())
}

fn encrypt_file(public_key_path: &Path, input: &Path, output: &Path) -> Result<()> {
  let public_key = EncapsulationKey::new(&ML_KEM_1024, &read_key(public_key_path, false)?)?;
  let (kem_ciphertext, shared_secret) = public_key.encapsulate()?;
  let mut plaintext = fs::read(input)?;

  let mut header = [0u8; HEADER_LEN];
  header[.. PREFIX.len()].copy_from_slice(PREFIX);
  let rng = SystemRandom::new();
  rng.fill(&mut header[PREFIX.len() .. SALT_END])?;
  header[SALT_END .. KEM_END].copy_from_slice(kem_ciphertext.as_ref());
  let mut nonce_bytes = [0u8; NONCE_LEN];
  rng.fill(&mut nonce_bytes)?;
  header[KEM_END ..].copy_from_slice(&nonce_bytes);

  let key = derive_key(shared_secret.as_ref(), &header[PREFIX.len() .. SALT_END])?;
  key.seal_in_place_append_tag(
    Nonce::assume_unique_for_key(nonce_bytes),
    Aad::from(&header),
    &mut plaintext,
  )?;

  let mut encrypted = header.to_vec();
  encrypted.extend_from_slice(&plaintext);
  write_new(output, &encrypted)
}

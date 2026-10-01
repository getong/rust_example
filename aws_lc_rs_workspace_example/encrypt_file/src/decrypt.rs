mod common;

use std::{env, fs, path::Path};

use aws_lc_rs::{
  aead::{AES_256_GCM, Aad, NONCE_LEN, Nonce},
  kem::{Ciphertext, DecapsulationKey, ML_KEM_1024},
};
use common::{HEADER_LEN, KEM_END, PREFIX, Result, SALT_END, derive_key, write_new};

const USAGE: &str =
  "用法：decrypt <私钥文件> <加密文件> <输出文件>\n      decrypt --keygen <公钥文件> <私钥文件>";

fn main() -> Result<()> {
  let args: Vec<_> = env::args_os().skip(1).collect();
  if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
    println!("{USAGE}");
    return Ok(());
  }
  if args.len() != 3 {
    return Err(USAGE.into());
  }
  if args[0] == "--keygen" {
    generate_keys(Path::new(&args[1]), Path::new(&args[2]))?;
    println!("密钥已生成。公钥用于加密，私钥请独立妥善保存。");
  } else {
    decrypt_file(
      Path::new(&args[0]),
      Path::new(&args[1]),
      Path::new(&args[2]),
    )?;
    println!("解密成功：{}", Path::new(&args[2]).display());
  }
  Ok(())
}

fn generate_keys(public_path: &Path, private_path: &Path) -> Result<()> {
  if public_path == private_path || public_path.try_exists()? || private_path.try_exists()? {
    return Err("公私钥必须使用不同的新文件路径；不会覆盖已有文件".into());
  }
  let private_key = DecapsulationKey::generate(&ML_KEM_1024)?;
  let public_bytes = private_key.encapsulation_key()?.key_bytes()?;
  let private_bytes = private_key.key_bytes()?;
  // 先保存私钥，即使公钥写入失败，也不丢失已生成的私钥。
  write_new(private_path, private_bytes.as_ref())?;
  write_new(public_path, public_bytes.as_ref())
}

fn decrypt_file(private_key_path: &Path, input: &Path, output: &Path) -> Result<()> {
  let mut encrypted = fs::read(input)?;
  if encrypted.len() < HEADER_LEN + AES_256_GCM.tag_len() || !encrypted.starts_with(PREFIX) {
    return Err("加密文件损坏，或文件版本/算法不受支持".into());
  }
  let private_key = DecapsulationKey::new(&ML_KEM_1024, &fs::read(private_key_path)?)?;
  let (header, ciphertext) = encrypted.split_at_mut(HEADER_LEN);
  let shared_secret = private_key.decapsulate(Ciphertext::from(&header[SALT_END .. KEM_END]))?;
  let key = derive_key(shared_secret.as_ref(), &header[PREFIX.len() .. SALT_END])?;
  let nonce_bytes: [u8; NONCE_LEN] = header[KEM_END ..].try_into()?;
  let plaintext = key
    .open_in_place(
      Nonce::assume_unique_for_key(nonce_bytes),
      Aad::from(&*header),
      ciphertext,
    )
    .map_err(|_| "解密认证失败：私钥不匹配或文件被修改")?;
  // 认证通过后才创建输出，不把未认证的明文写入磁盘。
  write_new(output, plaintext)
}

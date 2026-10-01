use std::{fs::OpenOptions, io::Write, path::Path};

use aws_lc_rs::{
  aead::{AES_256_GCM, LessSafeKey, UnboundKey},
  hkdf::{HKDF_SHA256, Salt},
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// v1 / suite 1: ML-KEM-1024 + HKDF-SHA256 + AES-256-GCM。
// 文件头整体作为 AAD，不能在不触发认证失败的情况下修改。
pub const PREFIX: &[u8; 10] = b"ALCFENC\0\x01\x01";
pub const SALT_LEN: usize = 32;
pub const KEM_CIPHERTEXT_LEN: usize = 1568;
pub const SALT_END: usize = PREFIX.len() + SALT_LEN;
pub const KEM_END: usize = SALT_END + KEM_CIPHERTEXT_LEN;
pub const HEADER_LEN: usize = KEM_END + aws_lc_rs::aead::NONCE_LEN;

pub fn derive_key(shared_secret: &[u8], salt: &[u8]) -> Result<LessSafeKey> {
  let salt = Salt::new(HKDF_SHA256, salt);
  let prk = salt.extract(shared_secret);
  let info = [b"encrypt_file/v1/ML-KEM-1024/HKDF-SHA256/AES-256-GCM".as_slice()];
  let okm = prk.expand(&info, &AES_256_GCM)?;
  Ok(LessSafeKey::new(UnboundKey::from(okm)))
}

/// 排他创建，避免覆盖输入、旧密钥或已有明文。Unix 下仅文件所有者可读写。
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
  let mut options = OpenOptions::new();
  options.write(true).create_new(true);
  #[cfg(unix)]
  {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
  }
  let mut file = options.open(path)?;
  file.write_all(bytes)?;
  file.sync_all()?;
  Ok(())
}

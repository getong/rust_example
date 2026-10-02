use std::{
  fs::{self, OpenOptions},
  io::{self, IsTerminal, Write},
  num::NonZeroU32,
  path::Path,
};

use aws_lc_rs::{
  aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
  hkdf::{HKDF_SHA256, Salt},
  pbkdf2,
  rand::{SecureRandom, SystemRandom},
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

const KEY_PREFIX: &[u8; 10] = b"ALCFKEY\0\x01\x01";
const KEY_HEADER_LEN: usize = KEY_PREFIX.len() + 1 + SALT_LEN + NONCE_LEN;

pub fn read_password(prompt: &str) -> Result<String> {
  if io::stdin().is_terminal() {
    return read_masked_password(prompt);
  }
  eprint!("{prompt}");
  io::stderr().flush()?;
  let mut password = String::new();
  if io::stdin().read_line(&mut password)? == 0 {
    return Err("未从 stdin 读到密码；不设置密码请直接按 Enter".into());
  }
  if password.ends_with('\n') {
    password.pop();
    if password.ends_with('\r') {
      password.pop();
    }
  }
  Ok(password)
}

fn read_masked_password(prompt: &str) -> Result<String> {
  use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
  };

  // Restore echo and normal terminal input even on errors or cancellation.
  struct RawModeGuard;
  impl Drop for RawModeGuard {
    fn drop(&mut self) {
      let _ = disable_raw_mode();
    }
  }
  enable_raw_mode()?;
  let _guard = RawModeGuard;
  let mut stderr = io::stderr().lock();
  write!(stderr, "{prompt}")?;
  stderr.flush()?;
  let mut password = String::new();
  loop {
    let Event::Key(key) = event::read()? else {
      continue;
    };
    if key.kind == KeyEventKind::Release {
      continue;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
      if matches!(key.code, KeyCode::Char('c' | 'd')) {
        write!(stderr, "\r\n")?;
        stderr.flush()?;
        return Err("密码输入已取消".into());
      }
      continue;
    }
    match key.code {
      KeyCode::Enter => {
        write!(stderr, "\r\n")?;
        stderr.flush()?;
        return Ok(password);
      }
      KeyCode::Backspace => {
        if password.pop().is_some() {
          write!(stderr, "\x08 \x08")?;
        }
      }
      KeyCode::Char(c) if !c.is_control() && !key.modifiers.contains(KeyModifiers::ALT) => {
        password.push(c);
        write!(stderr, "*")?;
      }
      _ => {}
    }
    stderr.flush()?;
  }
}

fn password_key(password: &str, salt: &[u8]) -> Result<LessSafeKey> {
  let mut bytes = [0u8; 32];
  pbkdf2::derive(
    pbkdf2::PBKDF2_HMAC_SHA256,
    NonZeroU32::new(600_000).unwrap(),
    salt,
    password.as_bytes(),
    &mut bytes,
  );
  Ok(LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &bytes)?))
}

// Only the decrypt binary generates key pairs; both binaries read this format.
#[allow(dead_code)]
pub fn protect_key(bytes: &[u8], private: bool, password: &str) -> Result<Vec<u8>> {
  if password.is_empty() {
    return Ok(bytes.to_vec());
  }
  let mut header = [0u8; KEY_HEADER_LEN];
  header[.. KEY_PREFIX.len()].copy_from_slice(KEY_PREFIX);
  header[KEY_PREFIX.len()] = u8::from(private);
  let salt_start = KEY_PREFIX.len() + 1;
  let nonce_start = salt_start + SALT_LEN;
  SystemRandom::new().fill(&mut header[salt_start ..])?;
  let key = password_key(password, &header[salt_start .. nonce_start])?;
  let nonce: [u8; NONCE_LEN] = header[nonce_start ..].try_into()?;
  let mut ciphertext = bytes.to_vec();
  key.seal_in_place_append_tag(
    Nonce::assume_unique_for_key(nonce),
    Aad::from(&header),
    &mut ciphertext,
  )?;
  let mut result = header.to_vec();
  result.extend_from_slice(&ciphertext);
  Ok(result)
}

pub fn read_key(path: &Path, private: bool) -> Result<Vec<u8>> {
  let mut bytes = fs::read(path)?;
  let raw_len = if private { 3168 } else { 1568 };
  if bytes.len() == raw_len {
    return Ok(bytes);
  }
  if bytes.len() != KEY_HEADER_LEN + raw_len + AES_256_GCM.tag_len()
    || !bytes.starts_with(KEY_PREFIX)
    || bytes[KEY_PREFIX.len()] != u8::from(private)
  {
    return Err("密钥文件损坏，或密钥类型/版本不受支持".into());
  }
  let password = read_password("请输入密钥密码：")?;
  if password.is_empty() {
    return Err("此密钥受密码保护，密码不能为空".into());
  }
  let (header, ciphertext) = bytes.split_at_mut(KEY_HEADER_LEN);
  let salt_start = KEY_PREFIX.len() + 1;
  let nonce_start = salt_start + SALT_LEN;
  let key = password_key(&password, &header[salt_start .. nonce_start])?;
  let nonce: [u8; NONCE_LEN] = header[nonce_start ..].try_into()?;
  let plaintext = key
    .open_in_place(
      Nonce::assume_unique_for_key(nonce),
      Aad::from(&*header),
      ciphertext,
    )
    .map_err(|_| "密钥密码错误或密钥文件被修改")?;
  Ok(plaintext.to_vec())
}

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

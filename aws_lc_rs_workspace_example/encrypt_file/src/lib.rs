#[cfg(not(windows))]
use std::fs::OpenOptions;
use std::{
  fs,
  io::{BufRead, IsTerminal, Read, Write, stderr, stdin},
  num::NonZeroU32,
  path::Path,
};

use argon2::{Algorithm, Argon2, Params, Version};
use aws_lc_rs::{
  aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
  hkdf::{HKDF_SHA256, Salt},
  kem::{Ciphertext, DecapsulationKey, EncapsulationKey, ML_KEM_1024},
  pbkdf2,
  rand::{SecureRandom, SystemRandom},
};
use inquire::{Password, PasswordDisplayMode};
use zeroize::Zeroizing;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// v1 / suite 1: ML-KEM-1024 + HKDF-SHA256 + AES-256-GCM。
// 文件头整体作为 AAD，不能在不触发认证失败的情况下修改。
pub const PREFIX: &[u8; 10] = b"ALCFENC\0\x01\x01";
pub const SALT_LEN: usize = 32;
pub const KEM_CIPHERTEXT_LEN: usize = 1568;
pub const SALT_END: usize = PREFIX.len() + SALT_LEN;
pub const KEM_END: usize = SALT_END + KEM_CIPHERTEXT_LEN;
pub const HEADER_LEN: usize = KEM_END + aws_lc_rs::aead::NONCE_LEN;

const LEGACY_KEY_PREFIX: &[u8; 10] = b"ALCFKEY\0\x01\x01";
const KEY_PREFIX: &[u8; 10] = b"ALCFKEY\0\x02\x01";
const PARAMS_LEN: usize = 12;
const KEY_HEADER_LEN: usize = KEY_PREFIX.len() + 1 + PARAMS_LEN + SALT_LEN + NONCE_LEN;
const LEGACY_HEADER_LEN: usize = KEY_HEADER_LEN - PARAMS_LEN;
// v1 has no encoded parameters. Never change this without a format version change.
const LEGACY_ITERATIONS: NonZeroU32 = NonZeroU32::new(600_000).unwrap();
pub const PRIVATE_KEY_LEN: usize = 3168;
pub const PUBLIC_KEY_LEN: usize = 1568;
pub const MAX_PLAINTEXT_LEN: u64 = 64 * 1024 * 1024;
const ARGON_MEMORY: u32 = 64 * 1024;
const ARGON_TIME: u32 = 3;
const ARGON_LANES: u32 = 1;

pub fn read_password(prompt: &str) -> Result<Zeroizing<String>> {
  if stdin().is_terminal() {
    let password = Zeroizing::new(
      Password::new(prompt)
        .with_display_mode(PasswordDisplayMode::Masked)
        .without_confirmation()
        .prompt()?,
    );
    if password.len() > 1024 {
      return Err("密码最多 1024 字节".into());
    }
    return Ok(password);
  }
  let mut stderr = stderr().lock();
  write!(stderr, "{prompt}")?;
  stderr.flush()?;
  let mut password = Zeroizing::new(String::with_capacity(1025));
  if stdin().lock().take(1025).read_line(&mut password)? == 0 {
    return Err("未从 stdin 读到密码；不设置密码请直接按 Enter".into());
  }
  if password.len() > 1024 {
    return Err("密码最多 1024 字节".into());
  }
  if password.ends_with('\n') {
    password.pop();
    if password.ends_with('\r') {
      password.pop();
    }
  }
  Ok(password)
}

fn password_key(
  password: &str,
  salt: &[u8],
  params: Option<(u32, u32, u32)>,
) -> Result<LessSafeKey> {
  let mut bytes = Zeroizing::new([0u8; 32]);
  if let Some((memory, time, lanes)) = params {
    let params = Params::new(memory, time, lanes, Some(32))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    // Explicitly erase the memory-hard workspace too, including on failure.
    let mut blocks = Zeroizing::new(vec![argon2::Block::default(); argon.params().block_count()]);
    argon.hash_password_into_with_memory(password.as_bytes(), salt, &mut *bytes, &mut *blocks)?;
  } else {
    pbkdf2::derive(
      pbkdf2::PBKDF2_HMAC_SHA256,
      LEGACY_ITERATIONS,
      salt,
      password.as_bytes(),
      &mut *bytes,
    );
  }
  Ok(LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &*bytes)?))
}

pub fn protect_private_key(bytes: &[u8], password: &str) -> Result<Zeroizing<Vec<u8>>> {
  if bytes.len() != PRIVATE_KEY_LEN {
    return Err("私钥长度错误".into());
  }
  if password.is_empty() {
    return Ok(Zeroizing::new(bytes.to_vec()));
  }
  let mut header = [0u8; KEY_HEADER_LEN];
  header[.. KEY_PREFIX.len()].copy_from_slice(KEY_PREFIX);
  header[10] = 1;
  for (field, value) in
    header[11 .. 23]
      .as_chunks_mut::<4>()
      .0
      .iter_mut()
      .zip([ARGON_MEMORY, ARGON_TIME, ARGON_LANES])
  {
    field.copy_from_slice(&value.to_le_bytes());
  }
  let salt_start = 11 + PARAMS_LEN;
  let nonce_start = salt_start + SALT_LEN;
  SystemRandom::new().fill(&mut header[salt_start ..])?;
  let key = password_key(
    password,
    &header[salt_start .. nonce_start],
    Some((ARGON_MEMORY, ARGON_TIME, ARGON_LANES)),
  )?;
  let nonce = header[nonce_start ..].try_into()?;
  let mut ciphertext = Zeroizing::new(Vec::with_capacity(bytes.len() + AES_256_GCM.tag_len()));
  ciphertext.extend_from_slice(bytes);
  key.seal_in_place_append_tag(
    Nonce::assume_unique_for_key(nonce),
    Aad::from(&header),
    &mut *ciphertext,
  )?;
  let mut result = Zeroizing::new(header.to_vec());
  result.extend_from_slice(&ciphertext);
  Ok(result)
}

pub fn read_key(path: &Path, private: bool) -> Result<Zeroizing<Vec<u8>>> {
  let mut bytes = read_bounded(
    path,
    (KEY_HEADER_LEN + PRIVATE_KEY_LEN + AES_256_GCM.tag_len()) as u64,
  )?;
  let raw_len = if private {
    PRIVATE_KEY_LEN
  } else {
    PUBLIC_KEY_LEN
  };
  if bytes.len() == raw_len {
    return Ok(bytes);
  }
  if !private {
    return Err("公钥必须使用裸格式；旧加密公钥请从对应私钥重新导出".into());
  }
  let (header_len, params) = if bytes.starts_with(KEY_PREFIX) && bytes.len() >= KEY_HEADER_LEN {
    let read = |offset| u32::from_le_bytes(bytes[offset .. offset + 4].try_into().unwrap());
    let (memory, time, lanes) = (read(11), read(15), read(19));
    // Unauthenticated parameters must be bounded before allocating memory / running KDF.
    if !(65536 ..= 262144).contains(&memory)
      || !(3 ..= 10).contains(&time)
      || !(1 ..= 4).contains(&lanes)
    {
      return Err("Argon2id 参数超出支持范围".into());
    }
    (KEY_HEADER_LEN, Some((memory, time, lanes)))
  } else if bytes.starts_with(LEGACY_KEY_PREFIX) {
    (LEGACY_HEADER_LEN, None)
  } else {
    return Err("密钥文件损坏，或密钥类型/版本不受支持".into());
  };
  if bytes.len() != header_len + raw_len + AES_256_GCM.tag_len() || bytes[10] != 1 {
    return Err("密钥文件损坏，或密钥类型/版本不受支持".into());
  }
  let password = read_password("请输入密钥密码：")?;
  if password.is_empty() {
    return Err("此密钥受密码保护，密码不能为空".into());
  }
  let (header, ciphertext) = bytes.split_at_mut(header_len);
  let nonce_start = header_len - NONCE_LEN;
  let salt_start = nonce_start - SALT_LEN;
  let key = password_key(&password, &header[salt_start .. nonce_start], params)?;
  let nonce = header[nonce_start ..].try_into()?;
  let plaintext = key
    .open_in_place(
      Nonce::assume_unique_for_key(nonce),
      Aad::from(&*header),
      ciphertext,
    )
    .map_err(|_| "密钥密码错误或密钥文件被修改")?;
  Ok(Zeroizing::new(plaintext.to_vec()))
}

/// Bound actual reads as well as metadata, including files growing during the read.
pub fn read_bounded(path: &Path, limit: u64) -> Result<Zeroizing<Vec<u8>>> {
  let file = fs::File::open(path)?;
  let metadata = file.metadata()?;
  if !metadata.is_file() || metadata.len() > limit {
    return Err("输入必须是大小不超过限制的普通文件".into());
  }
  // Reserve tag and sentinel room so in-place encryption does not copy plaintext on growth.
  let mut bytes = Zeroizing::new(Vec::with_capacity(metadata.len() as usize + 17));
  file.take(metadata.len() + 1).read_to_end(&mut bytes)?;
  if bytes.len() as u64 > metadata.len() {
    return Err("文件超过大小限制".into());
  }
  Ok(bytes)
}

pub fn derive_key(shared_secret: &[u8], salt: &[u8]) -> Result<LessSafeKey> {
  let salt = Salt::new(HKDF_SHA256, salt);
  let prk = salt.extract(shared_secret);
  // Map the on-disk version/suite to its exact legacy domain separation string.
  let domain: &[u8] = match (PREFIX[8], PREFIX[9]) {
    (1, 1) => b"encrypt_file/v1/ML-KEM-1024/HKDF-SHA256/AES-256-GCM",
    _ => return Err("不支持的算法套件".into()),
  };
  let info = [domain];
  let okm = prk.expand(&info, &AES_256_GCM)?;
  Ok(LessSafeKey::new(UnboundKey::from(okm)))
}

/// 排他创建，避免覆盖输入、旧密钥或已有明文。Unix 下仅文件所有者可读写。
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
  write_new_parts(path, &[bytes])
}

pub fn write_new_parts(path: &Path, parts: &[&[u8]]) -> Result<()> {
  #[cfg(not(windows))]
  let mut options = OpenOptions::new();
  #[cfg(not(windows))]
  options.write(true).create_new(true);
  #[cfg(unix)]
  {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
  }
  #[cfg(not(windows))]
  let mut file = options.open(path)?;
  #[cfg(windows)]
  let mut file = windows_create_new(path)?;
  for part in parts {
    file.write_all(part)?;
  }
  file.sync_all()?;
  #[cfg(unix)]
  fs::File::open(
    path
      .parent()
      .filter(|p| !p.as_os_str().is_empty())
      .unwrap_or(Path::new(".")),
  )?
  .sync_all()?;
  Ok(())
}

#[cfg(windows)]
fn windows_create_new(path: &Path) -> Result<fs::File> {
  use std::{
    io,
    os::windows::{ffi::OsStrExt, io::FromRawHandle},
  };

  use windows_sys::Win32::{
    Foundation::{GENERIC_WRITE, INVALID_HANDLE_VALUE, LocalFree},
    Security::{
      Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW, SECURITY_ATTRIBUTES,
    },
    Storage::FileSystem::{CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_NORMAL},
  };
  let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
  if path[.. path.len() - 1].contains(&0) {
    return Err("文件路径包含 NUL".into());
  }
  // Protected DACL: owner rights only, no inherited grants; applied at creation.
  let sddl: Vec<u16> = "D:P(A;;FA;;;OW)".encode_utf16().chain(Some(0)).collect();
  unsafe {
    let mut descriptor = std::ptr::null_mut();
    if ConvertStringSecurityDescriptorToSecurityDescriptorW(
      sddl.as_ptr(),
      1,
      &mut descriptor,
      std::ptr::null_mut(),
    ) == 0
    {
      return Err(io::Error::last_os_error().into());
    }
    let attributes = SECURITY_ATTRIBUTES {
      nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
      lpSecurityDescriptor: descriptor,
      bInheritHandle: 0,
    };
    let handle = CreateFileW(
      path.as_ptr(),
      GENERIC_WRITE,
      0,
      &attributes,
      CREATE_NEW,
      FILE_ATTRIBUTE_NORMAL,
      std::ptr::null_mut(),
    );
    let error = io::Error::last_os_error();
    LocalFree(descriptor);
    if handle == INVALID_HANDLE_VALUE {
      return Err(error.into());
    }
    Ok(fs::File::from_raw_handle(handle))
  }
}

pub fn encrypt_file(public_key_path: &Path, input: &Path, output: &Path) -> Result<()> {
  let public_key = EncapsulationKey::new(&ML_KEM_1024, &read_key(public_key_path, false)?)?;
  let (kem_ciphertext, shared_secret) = public_key.encapsulate()?;
  let mut plaintext = read_bounded(input, MAX_PLAINTEXT_LEN)?;

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
    &mut *plaintext,
  )?;

  write_new_parts(output, &[&header, &plaintext])
}

pub fn generate_keys(public_path: &Path, private_path: &Path) -> Result<()> {
  if public_path == private_path || public_path.try_exists()? || private_path.try_exists()? {
    return Err("公私钥必须使用不同的新文件路径；不会覆盖已有文件".into());
  }
  let password = read_password("请设置密钥密码（直接按 Enter 则不设密码）：")?;
  if !password.is_empty() {
    let confirmation = read_password("请再次输入密钥密码：")?;
    if password != confirmation {
      return Err("两次密码不一致，未生成密钥".into());
    }
  }
  let private_key = DecapsulationKey::generate(&ML_KEM_1024)?;
  let public_bytes = private_key.encapsulation_key()?.key_bytes()?;
  let private_bytes = private_key.key_bytes()?;
  let protected_private = protect_private_key(private_bytes.as_ref(), &password)?;
  // 先保存私钥，即使公钥写入失败，也不丢失已生成的私钥。
  write_new(private_path, &protected_private)?;
  write_new(public_path, public_bytes.as_ref())
}

pub fn decrypt_file(private_key_path: &Path, input: &Path, output: &Path) -> Result<()> {
  let mut encrypted = read_bounded(
    input,
    MAX_PLAINTEXT_LEN + HEADER_LEN as u64 + AES_256_GCM.tag_len() as u64,
  )?;
  if encrypted.len() < HEADER_LEN + AES_256_GCM.tag_len() || !encrypted.starts_with(PREFIX) {
    return Err("加密文件损坏，或文件版本/算法不受支持".into());
  }
  let private_key = DecapsulationKey::new(&ML_KEM_1024, &read_key(private_key_path, true)?)?;
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

#[cfg(test)]
mod tests {
  use aws_lc_rs::kem::{DecapsulationKey, ML_KEM_1024};

  use super::*;

  #[test]
  fn suite_sizes_match_backend() {
    // aws-lc-rs does not expose public algorithm size accessors.
    let private = DecapsulationKey::generate(&ML_KEM_1024).unwrap();
    let public = private.encapsulation_key().unwrap();
    assert_eq!(private.key_bytes().unwrap().as_ref().len(), PRIVATE_KEY_LEN);
    assert_eq!(public.key_bytes().unwrap().as_ref().len(), PUBLIC_KEY_LEN);
    assert_eq!(
      public.encapsulate().unwrap().0.as_ref().len(),
      KEM_CIPHERTEXT_LEN
    );
  }
}

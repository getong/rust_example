#[cfg(not(windows))]
use std::fs::OpenOptions;
use std::{
  fs,
  io::{Read, Write},
  path::Path,
};

use zeroize::Zeroizing;

use crate::{Error, Result, format::TAG_LEN};

/// Bound actual reads as well as metadata, including files growing during the read.
pub fn read_bounded(path: &Path, limit: u64) -> Result<Zeroizing<Vec<u8>>> {
  let _span = crate::perf::span("io.read");
  let file = fs::File::open(path)?;
  let metadata = file.metadata()?;
  if !metadata.is_file() || metadata.len() > limit {
    return Err(Error::InputLimit);
  }
  // Reserve tag and sentinel room so in-place encryption does not copy plaintext on growth.
  let capacity = usize::try_from(metadata.len())
    .ok()
    .and_then(|len| len.checked_add(TAG_LEN + 1))
    .ok_or(Error::InputLimit)?;
  let mut bytes = Zeroizing::new(Vec::new());
  bytes
    .try_reserve_exact(capacity)
    .map_err(|_| Error::InputLimit)?;
  file.take(metadata.len() + 1).read_to_end(&mut bytes)?;
  if bytes.len() as u64 != metadata.len() {
    return Err(Error::InputLimit);
  }
  Ok(bytes)
}

/// 排他创建，避免覆盖输入、旧密钥或已有明文。Unix 下仅文件所有者可读写。
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
  write_new_parts(path, &[bytes])
}

pub fn write_new_parts(path: &Path, parts: &[&[u8]]) -> Result<()> {
  let _span = crate::perf::span("io.write_total");
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
  let mut file = crate::platform::windows_create_new(path)?;
  for part in parts {
    measure!("io.write", file.write_all(part))?;
  }
  measure!("io.fsync_file", file.sync_all())?;
  #[cfg(unix)]
  measure!(
    "io.fsync_dir",
    fs::File::open(
      path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new(".")),
    )?
    .sync_all()
  )?;
  Ok(())
}

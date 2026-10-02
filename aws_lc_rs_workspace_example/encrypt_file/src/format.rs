use crate::{Error, Result};

pub const LEGACY_PREFIX: &[u8; 10] = b"ALCFENC\0\x01\x01";
pub const PREFIX: &[u8; 10] = b"ALCFENC\0\x02\x01";
pub const SALT_LEN: usize = 32;
pub const KEM_CIPHERTEXT_LEN: usize = 1568;
pub const SALT_END: usize = PREFIX.len() + SALT_LEN;
pub const KEM_END: usize = SALT_END + KEM_CIPHERTEXT_LEN;
pub const LEGACY_HEADER_LEN: usize = KEM_END + aws_lc_rs::aead::NONCE_LEN;
pub const SENDER_END: usize = LEGACY_HEADER_LEN + 32;
pub const RECIPIENT_END: usize = SENDER_END + 32;
pub const HEADER_LEN: usize = RECIPIENT_END + 8;
pub const TAG_LEN: usize = 16;
pub const SIGNATURE_LEN: usize = 4627;
pub const PRIVATE_KEY_LEN: usize = 3168;
pub const PUBLIC_KEY_LEN: usize = 1568;
pub const SIGN_PRIVATE_KEY_LEN: usize = 4896;
pub const SIGN_PUBLIC_KEY_LEN: usize = 2592;
pub const MAX_PLAINTEXT_LEN: u64 = 64 * 1024 * 1024;
pub const MAX_ENCRYPTED_LEN: u64 =
  MAX_PLAINTEXT_LEN + (HEADER_LEN + TAG_LEN + SIGNATURE_LEN) as u64;

pub(crate) const KEY_PREFIX: &[u8; 10] = b"ALCFKEY\0\x02\x01";
pub const KEY_HEADER_LEN: usize = 67;
pub(crate) const LEGACY_KEY_HEADER_LEN: usize = 55;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileVersion {
  Legacy,
  Signed,
}

#[derive(Debug)]
pub struct Envelope<'a> {
  pub version: FileVersion,
  pub header: &'a [u8],
  pub ciphertext: &'a [u8],
  pub signature: &'a [u8],
}

/// Structural validation only. No allocation, password prompt, KDF, or authentication.
pub fn parse_envelope(bytes: &[u8]) -> Result<Envelope<'_>> {
  if bytes.len() < PREFIX.len()
    || bytes.len() as u64 > MAX_ENCRYPTED_LEN
    || &bytes[.. 8] != b"ALCFENC\0"
  {
    return Err(Error::Corrupt);
  }
  let version = match bytes[8] {
    1 => FileVersion::Legacy,
    2 => FileVersion::Signed,
    n => return Err(Error::UnsupportedVersion(n)),
  };
  if bytes[9] != 1 {
    return Err(Error::UnsupportedSuite(bytes[9]));
  }
  let (header_len, signature_len) = match version {
    FileVersion::Legacy => (LEGACY_HEADER_LEN, 0),
    FileVersion::Signed => (HEADER_LEN, SIGNATURE_LEN),
  };
  if bytes.len() < header_len + TAG_LEN + signature_len {
    return Err(Error::Corrupt);
  }
  let plain_len = bytes.len() - header_len - TAG_LEN - signature_len;
  if plain_len as u64 > MAX_PLAINTEXT_LEN {
    return Err(Error::InputLimit);
  }
  if version == FileVersion::Signed {
    let encoded = u64::from_le_bytes(
      bytes[RECIPIENT_END .. HEADER_LEN]
        .try_into()
        .map_err(|_| Error::Corrupt)?,
    );
    if encoded != plain_len as u64 {
      return Err(Error::Corrupt);
    }
  }
  Ok(Envelope {
    version,
    header: &bytes[.. header_len],
    ciphertext: &bytes[header_len .. bytes.len() - signature_len],
    signature: &bytes[bytes.len() - signature_len ..],
  })
}

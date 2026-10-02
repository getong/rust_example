use crate::{Error, Result};

pub const LEGACY_PREFIX: &[u8; 10] = b"ALCFENC\0\x01\x01";
pub const V2_PREFIX: &[u8; 10] = b"ALCFENC\0\x02\x01";
pub const PREFIX: &[u8; 10] = b"ALCFENC\0\x03\x01";
pub const SALT_LEN: usize = 32;
pub const FINGERPRINT_LEN: usize = 32;
pub const NONCE_LEN: usize = aws_lc_rs::aead::NONCE_LEN;
pub const KEM_CIPHERTEXT_LEN: usize = 1568;
pub const SALT_END: usize = PREFIX.len() + SALT_LEN;
pub const KEM_END: usize = SALT_END + KEM_CIPHERTEXT_LEN;
pub const LEGACY_HEADER_LEN: usize = KEM_END + NONCE_LEN;
pub const SENDER_END: usize = LEGACY_HEADER_LEN + FINGERPRINT_LEN;
pub const RECIPIENT_END: usize = SENDER_END + FINGERPRINT_LEN;
pub const HEADER_LEN: usize = RECIPIENT_END + 8;
pub const TAG_LEN: usize = 16;
pub const SIGNATURE_LEN: usize = 4627;
pub const PRIVATE_KEY_LEN: usize = 3168;
pub const PUBLIC_KEY_LEN: usize = 1568;
// FIPS 203: dk = dkPKE || ek || SHA3-256(ek) || z (ML-KEM-1024, k=4).
pub const KEM_EMBEDDED_PUBLIC_START: usize = PRIVATE_KEY_LEN - PUBLIC_KEY_LEN - 2 * 32;
pub const KEM_EMBEDDED_PUBLIC_END: usize = KEM_EMBEDDED_PUBLIC_START + PUBLIC_KEY_LEN;
pub const KEM_EMBEDDED_HASH_END: usize = KEM_EMBEDDED_PUBLIC_END + 32;
pub const SIGN_PRIVATE_KEY_LEN: usize = 4896;
pub const SIGN_PUBLIC_KEY_LEN: usize = 2592;
pub const MAX_PLAINTEXT_LEN: u64 = 64 * 1024 * 1024;
pub const MAX_ENCRYPTED_LEN: u64 =
  MAX_PLAINTEXT_LEN + (HEADER_LEN + TAG_LEN + SIGNATURE_LEN) as u64;

pub(crate) const KEY_PREFIX: &[u8; 10] = b"ALCFKEY\0\x02\x01";
pub(crate) const KEY_TYPE_OFFSET: usize = KEY_PREFIX.len();
pub(crate) const KEY_PARAMS_START: usize = KEY_TYPE_OFFSET + 1;
pub(crate) const KEY_PARAM_LEN: usize = size_of::<u32>();
pub(crate) const KEY_SALT_START: usize = KEY_PARAMS_START + 3 * KEY_PARAM_LEN;
pub(crate) const KEY_NONCE_START: usize = KEY_SALT_START + SALT_LEN;
pub const KEY_HEADER_LEN: usize = KEY_NONCE_START + NONCE_LEN;
pub(crate) const LEGACY_KEY_HEADER_LEN: usize = KEY_PARAMS_START + SALT_LEN + NONCE_LEN;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileVersion {
  Legacy,
  /// v2: signature over a flat SHA-512 digest.
  Signed,
  /// v3: signature over the domain-separated parallel SHA-512 tree root.
  SignedTree,
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
  if bytes.len() < PREFIX.len() || bytes.len() as u64 > MAX_ENCRYPTED_LEN {
    return Err(Error::Corrupt);
  }
  let prefixes = [
    (LEGACY_PREFIX, FileVersion::Legacy),
    (V2_PREFIX, FileVersion::Signed),
    (PREFIX, FileVersion::SignedTree),
  ];
  let (expected, version) = prefixes
    .iter()
    .copied()
    .find(|(prefix, _)| bytes[.. 9] == prefix[.. 9])
    .ok_or_else(|| {
      if prefixes
        .iter()
        .any(|(prefix, _)| bytes[.. 8] == prefix[.. 8])
      {
        Error::UnsupportedVersion(bytes[8])
      } else {
        Error::Corrupt
      }
    })?;
  if !bytes.starts_with(expected) {
    return Err(Error::UnsupportedSuite(bytes[9]));
  }
  let (header_len, signature_len) = match version {
    FileVersion::Legacy => (LEGACY_HEADER_LEN, 0),
    FileVersion::Signed | FileVersion::SignedTree => (HEADER_LEN, SIGNATURE_LEN),
  };
  if bytes.len() < header_len + TAG_LEN + signature_len {
    return Err(Error::Corrupt);
  }
  let plain_len = bytes.len() - header_len - TAG_LEN - signature_len;
  if plain_len as u64 > MAX_PLAINTEXT_LEN {
    return Err(Error::InputLimit);
  }
  if version != FileVersion::Legacy {
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn known_prefixes_and_unknown_format_errors_are_consistent() {
    for (prefix, version, header_len, signature_len) in [
      (LEGACY_PREFIX, FileVersion::Legacy, LEGACY_HEADER_LEN, 0),
      (V2_PREFIX, FileVersion::Signed, HEADER_LEN, SIGNATURE_LEN),
      (PREFIX, FileVersion::SignedTree, HEADER_LEN, SIGNATURE_LEN),
    ] {
      // A structurally valid empty-plaintext envelope; authentication is a separate step.
      let mut bytes = vec![0; header_len + TAG_LEN + signature_len];
      bytes[.. prefix.len()].copy_from_slice(prefix);
      assert_eq!(parse_envelope(&bytes).unwrap().version, version);
      bytes[9] = 0xff;
      assert!(matches!(
        parse_envelope(&bytes),
        Err(Error::UnsupportedSuite(0xff))
      ));
      bytes[8] = 0xff;
      assert!(matches!(
        parse_envelope(&bytes),
        Err(Error::UnsupportedVersion(0xff))
      ));
      bytes[0] ^= 1;
      assert!(matches!(parse_envelope(&bytes), Err(Error::Corrupt)));
      assert!(matches!(
        parse_envelope(&prefix[.. prefix.len() - 1]),
        Err(Error::Corrupt)
      ));
    }
  }
}

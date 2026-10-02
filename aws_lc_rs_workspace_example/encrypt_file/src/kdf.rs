use std::num::NonZeroU32;

use argon2::{Algorithm, Argon2, Params, Version};
use aws_lc_rs::{
  aead::{AES_256_GCM, LessSafeKey, UnboundKey},
  hkdf::{HKDF_SHA256, Salt},
  pbkdf2,
};
use zeroize::Zeroizing;

use crate::{
  Error, Result,
  format::{FileVersion, SALT_END},
};

#[derive(Debug, Clone, Copy)]
pub struct ArgonParams {
  pub memory: u32,
  pub time: u32,
  pub lanes: u32,
}
impl ArgonParams {
  pub const DEFAULT: Self = Self {
    memory: 65536,
    time: 3,
    lanes: 1,
  };
  pub fn validate(self) -> Result<Self> {
    if !(65536 ..= 262144).contains(&self.memory)
      || !(3 ..= 10).contains(&self.time)
      || !(1 ..= 4).contains(&self.lanes)
    {
      return Err(Error::KdfParameters);
    }
    Ok(self)
  }
}

pub(crate) fn password_key(
  password: &str,
  salt: &[u8],
  params: Option<ArgonParams>,
) -> Result<LessSafeKey> {
  let _span = crate::perf::span("kdf.total");
  let mut bytes = Zeroizing::new([0u8; 32]);
  if let Some(params) = params {
    let p = params.validate()?;
    let argon = Argon2::new(
      Algorithm::Argon2id,
      Version::V0x13,
      Params::new(p.memory, p.time, p.lanes, Some(32))?,
    );
    let mut blocks = measure!(
      "kdf.allocate",
      Zeroizing::new(vec![argon2::Block::default(); argon.params().block_count()])
    );
    measure!(
      "kdf.argon2id",
      argon.hash_password_into_with_memory(password.as_bytes(), salt, &mut *bytes, &mut *blocks)
    )?;
    measure!("kdf.zeroize", drop(blocks));
  } else {
    // Frozen v1 on-disk contract, not a configurable password policy.
    pbkdf2::derive(
      pbkdf2::PBKDF2_HMAC_SHA256,
      NonZeroU32::new(600_000).unwrap(),
      salt,
      password.as_bytes(),
      &mut *bytes,
    );
  }
  Ok(LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &*bytes)?))
}

pub(crate) fn derive_key(
  secret: &[u8],
  header: &[u8],
  version: FileVersion,
) -> Result<LessSafeKey> {
  let _span = crate::perf::span("crypto.hkdf");
  let salt = Salt::new(HKDF_SHA256, &header[10 .. SALT_END]);
  let prk = salt.extract(secret);
  let domain: &[u8] = match version {
    FileVersion::Legacy => b"encrypt_file/v1/ML-KEM-1024/HKDF-SHA256/AES-256-GCM",
    FileVersion::Signed => b"encrypt_file/v2/ML-KEM-1024/HKDF-SHA256/AES-256-GCM/ML-DSA-87",
  };
  // v1 info is immutable; v2 binds the entire header, including the KEM ciphertext.
  let legacy = [domain];
  let signed = [domain, header];
  let info = if version == FileVersion::Legacy {
    &legacy[..]
  } else {
    &signed[..]
  };
  let okm = prk.expand(info, &AES_256_GCM)?;
  Ok(LessSafeKey::new(UnboundKey::from(okm)))
}

#[cfg(test)]
mod tests {
  use aws_lc_rs::aead::{Aad, Nonce};

  use super::*;
  use crate::format::{HEADER_LEN, SALT_END};
  #[test]
  fn v2_hkdf_binds_kem_ciphertext() {
    let header = [1u8; HEADER_LEN];
    let mut changed = header;
    changed[SALT_END] ^= 1;
    let original = derive_key(&[7; 32], &header, FileVersion::Signed).unwrap();
    let other = derive_key(&[7; 32], &changed, FileVersion::Signed).unwrap();
    let mut body = b"HKDF binding".to_vec();
    original
      .seal_in_place_append_tag(
        Nonce::assume_unique_for_key([0; 12]),
        Aad::empty(),
        &mut body,
      )
      .unwrap();
    assert!(
      other
        .open_in_place(
          Nonce::assume_unique_for_key([0; 12]),
          Aad::empty(),
          &mut body
        )
        .is_err()
    );
  }
}

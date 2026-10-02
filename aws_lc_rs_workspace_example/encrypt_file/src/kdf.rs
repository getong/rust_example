use std::num::NonZeroU32;

use argon2::{Algorithm, Argon2, Params, Version};
use aws_lc_rs::{
  aead::{AES_256_GCM, LessSafeKey, UnboundKey},
  hkdf::{HKDF_SHA256, Salt},
  pbkdf2,
};
use zeroize::{Zeroize, Zeroizing};

use crate::{
  Error, Result,
  format::{FileVersion, PREFIX, SALT_END},
};

#[derive(Debug, Clone, Copy)]
pub struct ArgonParams {
  pub memory: u32,
  pub time: u32,
  pub lanes: u32,
}
impl ArgonParams {
  /// Parameters written only to newly protected keys; existing headers are authoritative.
  pub fn for_new_key() -> Self {
    let lanes = std::thread::available_parallelism().map_or(1, |n| n.get().clamp(1, 4)) as u32;
    Self {
      memory: 65536,
      time: 3,
      lanes,
    }
  }
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

/// Operation-scoped memory: never shared between concurrent requests or kept in a static.
/// The entire initialized region is wiped on drop, including errors and unwinding.
#[derive(Default)]
pub(crate) struct KdfWorkspace {
  blocks: Vec<argon2::Block>,
}
impl KdfWorkspace {
  fn memory(&mut self, count: usize) -> Result<&mut [argon2::Block]> {
    if self.blocks.len() < count {
      // Do not reallocate/copy a dirty buffer. Wipe and free it before growing.
      self.wipe();
      self.blocks = Vec::new();
      measure!("kdf.allocate", {
        self
          .blocks
          .try_reserve_exact(count)
          .map_err(|_| Error::InputLimit)?;
        self.blocks.resize(count, argon2::Block::default());
      });
    }
    Ok(&mut self.blocks[.. count])
  }
  fn wipe(&mut self) {
    if !self.blocks.is_empty() {
      measure!("kdf.zeroize", {
        for block in &mut self.blocks {
          block.zeroize();
        }
      });
    }
  }
}
impl Drop for KdfWorkspace {
  fn drop(&mut self) {
    self.wipe();
  }
}

pub(crate) fn password_key(
  password: &str,
  salt: &[u8],
  params: Option<ArgonParams>,
  workspace: &mut KdfWorkspace,
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
    let blocks = workspace.memory(argon.params().block_count())?;
    // Argon2 initializes the first two blocks of every lane and overwrites the rest
    // on pass 0; previous contents are not input to the next derivation.
    measure!(
      "kdf.argon2id",
      argon.hash_password_into_with_memory(password.as_bytes(), salt, &mut *bytes, blocks)
    )?;
  } else {
    // Frozen v1 on-disk contract, not a configurable password policy.
    pbkdf2::derive(
      pbkdf2::PBKDF2_HMAC_SHA256,
      NonZeroU32::new(600_000).expect("legacy PBKDF2 iteration count is nonzero"),
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
  let salt = Salt::new(HKDF_SHA256, &header[PREFIX.len() .. SALT_END]);
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
  fn sealed_with(key: LessSafeKey) -> Vec<u8> {
    let mut body = b"workspace reuse".to_vec();
    key
      .seal_in_place_append_tag(
        Nonce::assume_unique_for_key([0; 12]),
        Aad::empty(),
        &mut body,
      )
      .unwrap();
    body
  }

  #[test]
  fn dirty_workspace_reuse_matches_fresh_memory_across_lanes_and_passwords() {
    let mut shared = KdfWorkspace::default();
    for (lanes, password) in [(4, "first"), (1, "second"), (3, "third"), (4, "fourth")] {
      let p = ArgonParams {
        memory: 65536,
        time: 3,
        lanes,
      };
      let salt = [lanes as u8; 32];
      let reused = password_key(password, &salt, Some(p), &mut shared).unwrap();
      let fresh = password_key(password, &salt, Some(p), &mut KdfWorkspace::default()).unwrap();
      assert_eq!(sealed_with(reused), sealed_with(fresh));
    }
    // Wipe covers the whole retained region, including any tail from a larger lane layout.
    shared.wipe();
    assert!(
      shared
        .blocks
        .iter()
        .all(|block| block.as_ref().iter().all(|word| *word == 0))
    );
  }

  #[test]
  fn growing_workspace_and_invalid_parameters_are_bounded() {
    let mut shared = KdfWorkspace::default();
    shared.memory(2).unwrap()[1].as_mut()[0] = 123;
    assert_eq!(shared.memory(1).unwrap().len(), 1);
    assert_eq!(shared.blocks.len(), 2);
    let grown = shared.memory(4).unwrap();
    assert!(
      grown
        .iter()
        .all(|block| block.as_ref().iter().all(|word| *word == 0))
    );
    assert!(matches!(
      password_key(
        "test",
        &[0; 32],
        Some(ArgonParams {
          memory: u32::MAX,
          time: 3,
          lanes: 4
        }),
        &mut shared
      ),
      Err(Error::KdfParameters)
    ));
    assert_eq!(shared.blocks.len(), 4);
    let new = ArgonParams::for_new_key();
    assert!((1 ..= 4).contains(&new.lanes));
    assert_eq!((new.memory, new.time), (65536, 3));
  }

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

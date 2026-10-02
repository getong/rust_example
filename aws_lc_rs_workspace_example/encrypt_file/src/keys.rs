use std::path::Path;

use aws_lc_rs::{
  aead::{Aad, Nonce},
  digest::{SHA256, digest},
  encoding::AsRawBytes,
  kem::{DecapsulationKey, ML_KEM_1024},
  rand::{SecureRandom, SystemRandom},
  signature::{KeyPair, ML_DSA_87_SIGNING, PqdsaKeyPair},
};
use zeroize::Zeroizing;

use crate::{
  Error, Result,
  file::{read_bounded, write_new},
  format::*,
  kdf::{ArgonParams, password_key},
  password::{PasswordSource, validate_password},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum KeyKind {
  KemPublic = 0,
  KemPrivate = 1,
  SignPublic = 2,
  SignPrivate = 3,
}
impl KeyKind {
  pub const fn raw_len(self) -> usize {
    match self {
      Self::KemPublic => PUBLIC_KEY_LEN,
      Self::KemPrivate => PRIVATE_KEY_LEN,
      Self::SignPublic => SIGN_PUBLIC_KEY_LEN,
      Self::SignPrivate => SIGN_PRIVATE_KEY_LEN,
    }
  }
}

/// Empty passwords cannot accidentally produce unprotected keys.
#[derive(Clone, Copy)]
pub enum Protection<'a> {
  Password(&'a str),
  Unprotected,
}

pub struct KeyEnvelope<'a> {
  pub raw: bool,
  pub header: &'a [u8],
  pub body: &'a [u8],
  pub params: Option<ArgonParams>,
}

/// Parse and bound unauthenticated KDF parameters without executing the KDF.
pub fn parse_key(bytes: &[u8], kind: KeyKind) -> Result<KeyEnvelope<'_>> {
  if bytes.len() == kind.raw_len() {
    return Ok(KeyEnvelope {
      raw: true,
      header: &[],
      body: bytes,
      params: None,
    });
  }
  if bytes.len() < 11 || &bytes[.. 8] != b"ALCFKEY\0" {
    return Err(Error::Corrupt);
  }
  if bytes[9] != 1 {
    return Err(Error::UnsupportedSuite(bytes[9]));
  }
  if bytes[10] != kind as u8 {
    return Err(Error::Corrupt);
  }
  let (header_len, params) = match bytes[8] {
    1 if kind == KeyKind::KemPrivate => (LEGACY_KEY_HEADER_LEN, None),
    2 => {
      if bytes.len() < KEY_HEADER_LEN {
        return Err(Error::Corrupt);
      }
      let read = |offset| -> Result<u32> {
        Ok(u32::from_le_bytes(
          bytes[offset .. offset + 4]
            .try_into()
            .map_err(|_| Error::Corrupt)?,
        ))
      };
      let params = ArgonParams {
        memory: read(11)?,
        time: read(15)?,
        lanes: read(19)?,
      }
      .validate()?;
      (KEY_HEADER_LEN, Some(params))
    }
    1 => return Err(Error::Corrupt),
    n => return Err(Error::UnsupportedVersion(n)),
  };
  if bytes.len() != header_len + kind.raw_len() + TAG_LEN {
    return Err(Error::Corrupt);
  }
  Ok(KeyEnvelope {
    raw: false,
    header: &bytes[.. header_len],
    body: &bytes[header_len ..],
    params,
  })
}

pub fn protect_key(
  bytes: &[u8],
  kind: KeyKind,
  protection: Protection<'_>,
) -> Result<Zeroizing<Vec<u8>>> {
  let _span = crate::perf::span("key.protect");
  if bytes.len() != kind.raw_len() {
    return Err(Error::Corrupt);
  }
  let Protection::Password(password) = protection else {
    return Ok(Zeroizing::new(bytes.to_vec()));
  };
  validate_password(password)?;
  let mut header = [0u8; KEY_HEADER_LEN];
  header[.. 10].copy_from_slice(KEY_PREFIX);
  header[10] = kind as u8;
  let p = ArgonParams::DEFAULT;
  for (offset, value) in [(11, p.memory), (15, p.time), (19, p.lanes)] {
    header[offset .. offset + 4].copy_from_slice(&value.to_le_bytes());
  }
  SystemRandom::new().fill(&mut header[23 ..])?;
  let key = password_key(password, &header[23 .. 55], Some(p))?;
  let nonce = Nonce::assume_unique_for_key(header[55 ..].try_into().map_err(|_| Error::Corrupt)?);
  let mut body = Zeroizing::new(Vec::with_capacity(bytes.len() + TAG_LEN));
  body.extend_from_slice(bytes);
  measure!(
    "crypto.key_encrypt",
    key.seal_in_place_append_tag(nonce, Aad::from(&header), &mut *body)
  )?;
  let mut output = Zeroizing::new(header.to_vec());
  output.extend_from_slice(&body);
  Ok(output)
}

pub fn unlock_key(
  mut bytes: Zeroizing<Vec<u8>>,
  kind: KeyKind,
  password: &str,
) -> Result<Zeroizing<Vec<u8>>> {
  let parsed = parse_key(&bytes, kind)?;
  if parsed.raw {
    return Ok(bytes);
  }
  validate_password(password)?;
  let header_len = parsed.header.len();
  let nonce_start = header_len - 12;
  let key = password_key(
    password,
    &parsed.header[nonce_start - 32 .. nonce_start],
    parsed.params,
  )?;
  let nonce = Nonce::assume_unique_for_key(
    parsed.header[nonce_start ..]
      .try_into()
      .map_err(|_| Error::Corrupt)?,
  );
  let (header, body) = bytes.split_at_mut(header_len);
  let plain = measure!(
    "crypto.key_decrypt",
    key.open_in_place(nonce, Aad::from(&*header), body)
  )
  .map_err(|_| Error::KeyUnlockFailed)?;
  Ok(Zeroizing::new(plain.to_vec()))
}

pub fn read_key(
  path: &Path,
  kind: KeyKind,
  passwords: &mut dyn PasswordSource,
) -> Result<Zeroizing<Vec<u8>>> {
  let _span = crate::perf::span("key.read_unlock");
  let bytes = read_bounded(path, (KEY_HEADER_LEN + kind.raw_len() + TAG_LEN) as u64)?;
  if parse_key(&bytes, kind)?.raw {
    return Ok(bytes);
  }
  let password = passwords.password(path, kind)?;
  unlock_key(bytes, kind, &password)
}

/// SHA-256 fingerprint of canonical raw public key bytes, never the encrypted wrapper.
pub fn fingerprint(bytes: &[u8]) -> [u8; 32] {
  let mut result = [0; 32];
  result.copy_from_slice(digest(&SHA256, bytes).as_ref());
  result
}
pub fn fingerprint_hex(bytes: &[u8]) -> String {
  fingerprint(bytes)
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect()
}

fn new_paths(public: &Path, private: &Path) -> Result<()> {
  if public == private || public.try_exists()? || private.try_exists()? {
    return Err(Error::OutputExists);
  }
  Ok(())
}

pub fn generate_keys(
  public: &Path,
  private: &Path,
  protection: Protection<'_>,
  protect_public: bool,
) -> Result<String> {
  let _span = crate::perf::span("command.keygen");
  new_paths(public, private)?;
  if let Protection::Password(p) = protection {
    validate_password(p)?;
  }
  let key = measure!(
    "crypto.kem_keygen",
    DecapsulationKey::generate(&ML_KEM_1024)
  )?;
  let public_bytes = key.encapsulation_key()?.key_bytes()?;
  let private_bytes = key.key_bytes()?;
  let encrypted_private = protect_key(private_bytes.as_ref(), KeyKind::KemPrivate, protection)?;
  let encrypted_public = protect_key(
    public_bytes.as_ref(),
    KeyKind::KemPublic,
    if protect_public {
      protection
    } else {
      Protection::Unprotected
    },
  )?;
  write_new(private, &encrypted_private)?;
  write_new(public, &encrypted_public)?;
  Ok(fingerprint_hex(public_bytes.as_ref()))
}

pub fn generate_signing_keys(
  public: &Path,
  private: &Path,
  protection: Protection<'_>,
) -> Result<String> {
  let _span = crate::perf::span("command.sign_keygen");
  new_paths(public, private)?;
  if let Protection::Password(p) = protection {
    validate_password(p)?;
  }
  let key = measure!(
    "crypto.sign_keygen",
    PqdsaKeyPair::generate(&ML_DSA_87_SIGNING)
  )?;
  let private_bytes = key.private_key().as_raw_bytes()?;
  let encrypted = protect_key(private_bytes.as_ref(), KeyKind::SignPrivate, protection)?;
  write_new(private, &encrypted)?;
  write_new(public, key.public_key().as_ref())?;
  Ok(fingerprint_hex(key.public_key().as_ref()))
}

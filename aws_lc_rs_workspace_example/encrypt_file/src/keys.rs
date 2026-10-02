use std::path::Path;

use aws_lc_rs::{
  aead::{Aad, Nonce},
  digest::{SHA3_256, SHA256, digest},
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
  kdf::{ArgonParams, KdfWorkspace, password_key},
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
  if bytes.len() < KEY_PARAMS_START || &bytes[.. 8] != b"ALCFKEY\0" {
    return Err(Error::Corrupt);
  }
  if bytes[9] != 1 {
    return Err(Error::UnsupportedSuite(bytes[9]));
  }
  if bytes[KEY_TYPE_OFFSET] != kind as u8 {
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
          bytes[offset .. offset + KEY_PARAM_LEN]
            .try_into()
            .map_err(|_| Error::Corrupt)?,
        ))
      };
      let params = ArgonParams {
        memory: read(KEY_PARAMS_START)?,
        time: read(KEY_PARAMS_START + KEY_PARAM_LEN)?,
        lanes: read(KEY_PARAMS_START + 2 * KEY_PARAM_LEN)?,
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
  protect_key_with_workspace(bytes, kind, protection, &mut KdfWorkspace::default())
}

fn protect_key_with_workspace(
  bytes: &[u8],
  kind: KeyKind,
  protection: Protection<'_>,
  workspace: &mut KdfWorkspace,
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
  header[.. KEY_PREFIX.len()].copy_from_slice(KEY_PREFIX);
  header[KEY_TYPE_OFFSET] = kind as u8;
  let p = ArgonParams::for_new_key();
  for (offset, value) in [
    (KEY_PARAMS_START, p.memory),
    (KEY_PARAMS_START + KEY_PARAM_LEN, p.time),
    (KEY_PARAMS_START + 2 * KEY_PARAM_LEN, p.lanes),
  ] {
    header[offset .. offset + KEY_PARAM_LEN].copy_from_slice(&value.to_le_bytes());
  }
  SystemRandom::new().fill(&mut header[KEY_SALT_START ..])?;
  let key = password_key(
    password,
    &header[KEY_SALT_START .. KEY_NONCE_START],
    Some(p),
    workspace,
  )?;
  let nonce = Nonce::assume_unique_for_key(
    header[KEY_NONCE_START ..]
      .try_into()
      .map_err(|_| Error::Corrupt)?,
  );
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
  bytes: Zeroizing<Vec<u8>>,
  kind: KeyKind,
  password: &str,
) -> Result<Zeroizing<Vec<u8>>> {
  let parsed = parse_key(&bytes, kind)?;
  if parsed.raw {
    return Ok(bytes);
  }
  let (header_len, params) = (parsed.header.len(), parsed.params);
  unlock_parsed(
    bytes,
    header_len,
    params,
    password,
    &mut KdfWorkspace::default(),
  )
}

// Only callers that have already validated the complete key envelope can enter here.
fn unlock_parsed(
  mut bytes: Zeroizing<Vec<u8>>,
  header_len: usize,
  params: Option<ArgonParams>,
  password: &str,
  workspace: &mut KdfWorkspace,
) -> Result<Zeroizing<Vec<u8>>> {
  validate_password(password)?;
  let nonce_start = header_len - NONCE_LEN;
  let key = password_key(
    password,
    &bytes[nonce_start - SALT_LEN .. nonce_start],
    params,
    workspace,
  )?;
  let nonce = Nonce::assume_unique_for_key(
    bytes[nonce_start .. header_len]
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
  read_key_with_workspace(path, kind, passwords, &mut KdfWorkspace::default())
}

/// Read keys in request order, reusing Argon2 memory within this batch.
///
/// The workspace is wiped before returning (including errors and panic unwinding).
/// An error aborts the batch and drops/zeroizes all keys already read; no partial
/// result is returned. Different key passwords can be supplied by a custom source.
///
/// ```no_run
/// use std::path::Path;
///
/// use encrypt_file::{FixedPassword, KeyKind, read_keys};
/// use zeroize::Zeroizing;
/// # fn example() -> encrypt_file::Result<()> {
/// let mut passwords = FixedPassword(Zeroizing::new("example-password".to_owned()));
/// let keys = read_keys(
///   &[
///     (Path::new("recipient.private"), KeyKind::KemPrivate),
///     (Path::new("sender.private"), KeyKind::SignPrivate),
///   ],
///   &mut passwords,
/// )?;
/// # Ok(())
/// # }
/// ```
pub fn read_keys(
  requests: &[(&Path, KeyKind)],
  passwords: &mut dyn PasswordSource,
) -> Result<Vec<Zeroizing<Vec<u8>>>> {
  let mut workspace = KdfWorkspace::default();
  requests
    .iter()
    .map(|&(path, kind)| read_key_with_workspace(path, kind, passwords, &mut workspace))
    .collect()
}

pub(crate) fn read_key_with_workspace(
  path: &Path,
  kind: KeyKind,
  passwords: &mut dyn PasswordSource,
  workspace: &mut KdfWorkspace,
) -> Result<Zeroizing<Vec<u8>>> {
  let _span = crate::perf::span("key.read_unlock");
  let bytes = read_bounded(path, (KEY_HEADER_LEN + kind.raw_len() + TAG_LEN) as u64)?;
  let parsed = parse_key(&bytes, kind)?;
  if parsed.raw {
    return Ok(bytes);
  }
  let (header_len, params) = (parsed.header.len(), parsed.params);
  let password = passwords.password(path, kind)?;
  unlock_parsed(bytes, header_len, params, &password, workspace)
}

/// Extract the embedded public key from FIPS 203's expanded ML-KEM-1024 private key.
/// AWS-LC's encapsulation_key() cannot export it from an imported DecapsulationKey.
/// Validate H(ek) using SHA3-256, distinct from our displayed SHA-256 fingerprint.
pub fn kem_public_from_private(private: &[u8]) -> Result<&[u8]> {
  if private.len() != PRIVATE_KEY_LEN {
    return Err(Error::Corrupt);
  }
  let public = &private[KEM_EMBEDDED_PUBLIC_START .. KEM_EMBEDDED_PUBLIC_END];
  if digest(&SHA3_256, public).as_ref()
    != &private[KEM_EMBEDDED_PUBLIC_END .. KEM_EMBEDDED_HASH_END]
  {
    return Err(Error::Corrupt);
  }
  Ok(public)
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

/// Preflight a pair of output paths before asking for a password.
///
/// This does not reserve paths. The final writes still use exclusive creation,
/// so files created after this check will never be overwritten.
#[must_use]
pub struct KeyOutputPaths<'a> {
  public: &'a Path,
  private: &'a Path,
}

impl<'a> KeyOutputPaths<'a> {
  pub fn new(public: &'a Path, private: &'a Path) -> Result<Self> {
    if public == private || public.try_exists()? || private.try_exists()? {
      return Err(Error::OutputExists);
    }
    Ok(Self { public, private })
  }

  pub fn generate_keys(self, protection: Protection<'_>, protect_public: bool) -> Result<String> {
    let Self { public, private } = self;
    let _span = crate::perf::span("command.keygen");
    if let Protection::Password(p) = protection {
      validate_password(p)?;
    }
    let key = measure!(
      "crypto.kem_keygen",
      DecapsulationKey::generate(&ML_KEM_1024)
    )?;
    let public_bytes = key.encapsulation_key()?.key_bytes()?;
    let private_bytes = key.key_bytes()?;
    let mut workspace = KdfWorkspace::default();
    let encrypted_private = protect_key_with_workspace(
      private_bytes.as_ref(),
      KeyKind::KemPrivate,
      protection,
      &mut workspace,
    )?;
    let encrypted_public = protect_key_with_workspace(
      public_bytes.as_ref(),
      KeyKind::KemPublic,
      if protect_public {
        protection
      } else {
        Protection::Unprotected
      },
      &mut workspace,
    )?;
    drop(workspace);
    write_new(private, &encrypted_private)?;
    write_new(public, &encrypted_public)?;
    Ok(fingerprint_hex(public_bytes.as_ref()))
  }

  pub fn generate_signing_keys(self, protection: Protection<'_>) -> Result<String> {
    let Self { public, private } = self;
    let _span = crate::perf::span("command.sign_keygen");
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
}

pub fn generate_keys(
  public: &Path,
  private: &Path,
  protection: Protection<'_>,
  protect_public: bool,
) -> Result<String> {
  KeyOutputPaths::new(public, private)?.generate_keys(protection, protect_public)
}

pub fn generate_signing_keys(
  public: &Path,
  private: &Path,
  protection: Protection<'_>,
) -> Result<String> {
  KeyOutputPaths::new(public, private)?.generate_signing_keys(protection)
}

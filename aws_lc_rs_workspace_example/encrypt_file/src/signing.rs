use aws_lc_rs::{
  digest::{SHA512, digest},
  signature::{KeyPair, ML_DSA_87, PqdsaKeyPair, UnparsedPublicKey},
};

use crate::{Error, Result, format::*, keys::fingerprint};

/// A trust decision made by the caller, never inferred from the encrypted file.
#[derive(Clone, Copy)]
pub enum Verification<'a> {
  Trusted(&'a [u8]),
  /// Only accepts old v1 files. Never bypasses signatures on v2 files.
  AllowUnsignedLegacy,
}

fn transcript(header: &[u8], ciphertext: &[u8]) -> Vec<u8> {
  // Pure ML-DSA over a protocol-specific transcript, NOT HashML-DSA.
  let mut message = b"encrypt_file/v2/signature/ML-DSA-87/SHA-512\0".to_vec();
  message.extend_from_slice(header);
  message.extend_from_slice(digest(&SHA512, ciphertext).as_ref());
  message
}

pub(crate) fn sign(header: &[u8], ciphertext: &[u8], key: &PqdsaKeyPair) -> Result<Vec<u8>> {
  let _span = crate::perf::span("crypto.sign");
  let mut signature = vec![0; SIGNATURE_LEN];
  let len = key.sign(&transcript(header, ciphertext), &mut signature)?;
  if len != SIGNATURE_LEN {
    return Err(Error::Corrupt);
  }
  Ok(signature)
}

pub fn verify_envelope(bytes: &[u8], verification: Verification<'_>) -> Result<()> {
  let _span = crate::perf::span("crypto.verify");
  let envelope = parse_envelope(bytes)?;
  match (envelope.version, verification) {
    (FileVersion::Legacy, Verification::AllowUnsignedLegacy) => Ok(()),
    (FileVersion::Signed, Verification::Trusted(public)) => {
      if public.len() != SIGN_PUBLIC_KEY_LEN
        || envelope.header[LEGACY_HEADER_LEN .. SENDER_END] != fingerprint(public)
      {
        return Err(Error::SignatureInvalid);
      }
      UnparsedPublicKey::new(&ML_DSA_87, public)
        .verify(
          &transcript(envelope.header, envelope.ciphertext),
          envelope.signature,
        )
        .map_err(|_| Error::SignatureInvalid)
    }
    _ => Err(Error::SignatureRequired),
  }
}

pub(crate) fn sender_fingerprint(key: &PqdsaKeyPair) -> [u8; 32] {
  fingerprint(key.public_key().as_ref())
}

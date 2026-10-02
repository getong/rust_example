use aws_lc_rs::{
  digest::{Context, SHA512, digest},
  signature::{KeyPair, ML_DSA_87, PqdsaKeyPair, UnparsedPublicKey},
};
use rayon::prelude::*;

use crate::{Error, Result, format::*, keys::fingerprint};

// Fixed protocol parameter; changing it requires another file format version.
pub const SIGNING_CHUNK_LEN: usize = 1024 * 1024;
const LEAF_DOMAIN: &[u8] = b"encrypt_file/v3/SHA-512-TREE-1M/leaf\0";
const ROOT_DOMAIN: &[u8] = b"encrypt_file/v3/SHA-512-TREE-1M/root\0";
const SIGN_DOMAIN: &[u8] = b"encrypt_file/v3/signature/ML-DSA-87/SHA-512-TREE-1M\0";

/// A trust decision made by the caller, never inferred from the encrypted file.
#[derive(Clone, Copy)]
pub enum Verification<'a> {
  Trusted(&'a [u8]),
  /// Only accepts old v1 files. Never bypasses signatures on v2/v3 files.
  AllowUnsignedLegacy,
}

fn leaf_digest(index: usize, chunk: &[u8]) -> [u8; 64] {
  let mut hash = Context::new(&SHA512);
  hash.update(LEAF_DOMAIN);
  hash.update(&(index as u64).to_le_bytes());
  hash.update(&(chunk.len() as u64).to_le_bytes());
  hash.update(chunk);
  let mut output = [0; 64];
  output.copy_from_slice(hash.finish().as_ref());
  output
}

fn tree_digest(ciphertext: &[u8]) -> [u8; 64] {
  // Indexed parallel iteration preserves leaf order. Read the original slices directly;
  // at the 64 MiB limit the only extra data is 65 * 64 bytes of leaf hashes.
  let leaves = if ciphertext.len() <= SIGNING_CHUNK_LEN {
    vec![leaf_digest(0, ciphertext)]
  } else {
    ciphertext
      .par_chunks(SIGNING_CHUNK_LEN)
      .enumerate()
      .map(|(index, chunk)| leaf_digest(index, chunk))
      .collect::<Vec<_>>()
  };
  let mut root = Context::new(&SHA512);
  root.update(ROOT_DOMAIN);
  root.update(&(ciphertext.len() as u64).to_le_bytes());
  root.update(&(SIGNING_CHUNK_LEN as u64).to_le_bytes());
  root.update(&(leaves.len() as u64).to_le_bytes());
  for leaf in leaves {
    root.update(&leaf);
  }
  let mut output = [0; 64];
  output.copy_from_slice(root.finish().as_ref());
  output
}

fn transcript(header: &[u8], ciphertext: &[u8], version: FileVersion) -> Result<Vec<u8>> {
  // Both signed formats bind ALL ciphertext bytes, including the GCM tag.
  // Signing only a tag would let a key-holding recipient forge sender-authenticated content.
  let (domain, hash): (&[u8], [u8; 64]) = match version {
    FileVersion::Legacy => return Err(Error::SignatureRequired),
    FileVersion::Signed => {
      let mut hash = [0; 64];
      hash.copy_from_slice(digest(&SHA512, ciphertext).as_ref());
      (b"encrypt_file/v2/signature/ML-DSA-87/SHA-512\0", hash)
    }
    FileVersion::SignedTree => (SIGN_DOMAIN, tree_digest(ciphertext)),
  };
  let mut message = Vec::with_capacity(domain.len() + header.len() + hash.len());
  message.extend_from_slice(domain);
  message.extend_from_slice(header);
  message.extend_from_slice(&hash);
  Ok(message)
}

pub(crate) fn sign(header: &[u8], ciphertext: &[u8], key: &PqdsaKeyPair) -> Result<Vec<u8>> {
  let _span = crate::perf::span("crypto.sign");
  let message = measure!(
    "crypto.signature_hash",
    transcript(header, ciphertext, FileVersion::SignedTree)
  )?;
  let mut signature = vec![0; SIGNATURE_LEN];
  let len = key.sign(&message, &mut signature)?;
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
    (FileVersion::Signed | FileVersion::SignedTree, Verification::Trusted(public)) => {
      if public.len() != SIGN_PUBLIC_KEY_LEN
        || envelope.header[LEGACY_HEADER_LEN .. SENDER_END] != fingerprint(public)
      {
        return Err(Error::SignatureInvalid);
      }
      let message = measure!(
        "crypto.signature_hash",
        transcript(envelope.header, envelope.ciphertext, envelope.version)
      )?;
      UnparsedPublicKey::new(&ML_DSA_87, public)
        .verify(&message, envelope.signature)
        .map_err(|_| Error::SignatureInvalid)
    }
    _ => Err(Error::SignatureRequired),
  }
}

pub(crate) fn sender_fingerprint(key: &PqdsaKeyPair) -> [u8; 32] {
  fingerprint(key.public_key().as_ref())
}

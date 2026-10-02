use std::path::Path;

use aws_lc_rs::{
  aead::{Aad, Nonce},
  kem::{Ciphertext, DecapsulationKey, EncapsulationKey, ML_KEM_1024},
  rand::{SecureRandom, SystemRandom},
  signature::{ML_DSA_87_SIGNING, PqdsaKeyPair},
};
use zeroize::{Zeroize, Zeroizing};

use crate::{
  Error, Result,
  file::{read_bounded, write_new, write_new_parts},
  format::*,
  kdf::derive_key,
  keys::{KeyKind, fingerprint, kem_public_from_private, read_key, read_keys},
  password::PasswordSource,
  signing::{self, Verification, verify_envelope},
};

pub struct EncryptedFile {
  pub header: [u8; HEADER_LEN],
  pub ciphertext: Zeroizing<Vec<u8>>,
  pub signature: Vec<u8>,
}
impl EncryptedFile {
  /// Useful for network callers/tests; file encryption writes parts without this copy.
  pub fn to_bytes(&self) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HEADER_LEN + self.ciphertext.len() + self.signature.len());
    bytes.extend_from_slice(&self.header);
    bytes.extend_from_slice(&self.ciphertext);
    bytes.extend_from_slice(&self.signature);
    bytes
  }
}

pub fn encrypt_bytes(
  mut plaintext: Zeroizing<Vec<u8>>,
  public: &[u8],
  signer: &PqdsaKeyPair,
) -> Result<EncryptedFile> {
  if plaintext.len() as u64 > MAX_PLAINTEXT_LEN {
    return Err(Error::InputLimit);
  }
  let public_key = EncapsulationKey::new(&ML_KEM_1024, public)?;
  let (kem, shared) = measure!("crypto.kem_encapsulate", public_key.encapsulate())?;
  let mut header = [0; HEADER_LEN];
  header[.. PREFIX.len()].copy_from_slice(PREFIX);
  SystemRandom::new().fill(&mut header[PREFIX.len() .. SALT_END])?;
  header[SALT_END .. KEM_END].copy_from_slice(kem.as_ref());
  SystemRandom::new().fill(&mut header[KEM_END .. LEGACY_HEADER_LEN])?;
  header[LEGACY_HEADER_LEN .. SENDER_END].copy_from_slice(&signing::sender_fingerprint(signer));
  header[SENDER_END .. RECIPIENT_END].copy_from_slice(&fingerprint(public));
  header[RECIPIENT_END .. HEADER_LEN].copy_from_slice(&(plaintext.len() as u64).to_le_bytes());
  let key = derive_key(shared.as_ref(), &header, FileVersion::SignedTree)?;
  let nonce = Nonce::assume_unique_for_key(
    header[KEM_END .. LEGACY_HEADER_LEN]
      .try_into()
      .map_err(|_| Error::Corrupt)?,
  );
  measure!(
    "crypto.file_encrypt",
    key.seal_in_place_append_tag(nonce, Aad::from(&header), &mut *plaintext)
  )?;
  let signature = signing::sign(&header, &plaintext, signer)?;
  Ok(EncryptedFile {
    header,
    ciphertext: plaintext,
    signature,
  })
}

/// Verifies the signature before KEM decapsulation. On failure no plaintext escapes.
pub fn decrypt_bytes(
  mut encrypted: Zeroizing<Vec<u8>>,
  private: &DecapsulationKey,
  verification: Verification<'_>,
) -> Result<Zeroizing<Vec<u8>>> {
  verify_envelope(&encrypted, verification)?;
  decrypt_verified(&mut encrypted, private)?;
  Ok(encrypted)
}

fn decrypt_verified(bytes: &mut Vec<u8>, private: &DecapsulationKey) -> Result<()> {
  let envelope = parse_envelope(bytes)?;
  if envelope.version != FileVersion::Legacy {
    let private_bytes = private.key_bytes()?;
    let public = kem_public_from_private(private_bytes.as_ref())?;
    if envelope.header[SENDER_END .. RECIPIENT_END] != fingerprint(public) {
      return Err(Error::RecipientMismatch);
    }
  }
  let shared = measure!(
    "crypto.kem_decapsulate",
    private.decapsulate(Ciphertext::from(&envelope.header[SALT_END .. KEM_END]))
  )?;
  let key = derive_key(shared.as_ref(), envelope.header, envelope.version)?;
  let nonce = Nonce::assume_unique_for_key(
    envelope.header[KEM_END .. LEGACY_HEADER_LEN]
      .try_into()
      .map_err(|_| Error::Corrupt)?,
  );
  let header_len = envelope.header.len();
  let body_len = envelope.ciphertext.len();
  let (header, body) = bytes.split_at_mut(header_len);
  let plain_len = measure!(
    "crypto.file_decrypt",
    key.open_in_place(nonce, Aad::from(&*header), &mut body[.. body_len])
  )
  .map_err(|_| Error::AuthenticationFailed)?
  .len();
  bytes.copy_within(header_len .. header_len + plain_len, 0);
  bytes[plain_len ..].zeroize();
  bytes.truncate(plain_len);
  Ok(())
}

pub fn encrypt_file(
  public_path: &Path,
  signer_path: &Path,
  input: &Path,
  output: &Path,
  passwords: &mut dyn PasswordSource,
) -> Result<()> {
  let _span = crate::perf::span("command.encrypt");
  // The batch releases its shared KDF workspace before the file buffer is allocated.
  let [public, signing_private]: [Zeroizing<Vec<u8>>; 2] = read_keys(
    &[
      (public_path, KeyKind::KemPublic),
      (signer_path, KeyKind::SignPrivate),
    ],
    passwords,
  )?
  .try_into()
  .map_err(|_| Error::Corrupt)?;
  let signer = PqdsaKeyPair::from_raw_private_key(&ML_DSA_87_SIGNING, &signing_private)?;
  let plaintext = read_bounded(input, MAX_PLAINTEXT_LEN)?;
  let sealed = encrypt_bytes(plaintext, &public, &signer)?;
  let result = write_new_parts(
    output,
    &[&sealed.header, &sealed.ciphertext, &sealed.signature],
  );
  measure!("memory.zeroize_file", drop(sealed));
  result
}

pub fn decrypt_file(
  private_path: &Path,
  input: &Path,
  output: &Path,
  verification: Verification<'_>,
  passwords: &mut dyn PasswordSource,
) -> Result<()> {
  let _span = crate::perf::span("command.decrypt");
  let mut encrypted = read_bounded(input, MAX_ENCRYPTED_LEN)?;
  // Reject unauthenticated files before password prompting / expensive key unlocking.
  verify_envelope(&encrypted, verification)?;
  let raw_key = read_key(private_path, KeyKind::KemPrivate, passwords)?;
  let private = DecapsulationKey::new(&ML_KEM_1024, &raw_key)?;
  decrypt_verified(&mut encrypted, &private)?;
  let result = write_new(output, &encrypted);
  measure!("memory.zeroize_file", drop(encrypted));
  result
}

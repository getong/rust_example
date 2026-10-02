#![no_main]
use std::sync::LazyLock;

use aws_lc_rs::{
  kem::{DecapsulationKey, ML_KEM_1024},
  signature::{KeyPair, ML_DSA_87_SIGNING, PqdsaKeyPair},
};
use encrypt_file::{format::parse_envelope, keys::parse_key, *};
use libfuzzer_sys::fuzz_target;
use zeroize::Zeroizing;

static PRIVATE: LazyLock<DecapsulationKey> = LazyLock::new(|| {
  DecapsulationKey::new(
    &ML_KEM_1024,
    include_bytes!("../../tests/fixtures/v1.private"),
  )
  .unwrap()
});
static SIGNER: LazyLock<PqdsaKeyPair> =
  LazyLock::new(|| PqdsaKeyPair::from_seed(&ML_DSA_87_SIGNING, &[23; 32]).unwrap());

fuzz_target!(|data: &[u8]| {
  // Keep executions cheap; malformed key headers are parsed without running Argon2.
  if data.len() > 128 * 1024 {
    return;
  }
  let _ = parse_envelope(data);
  for kind in [
    KeyKind::KemPublic,
    KeyKind::KemPrivate,
    KeyKind::SignPublic,
    KeyKind::SignPrivate,
  ] {
    let _ = parse_key(data, kind);
  }
  let verifier: &[u8] = if data.get(8) == Some(&2) {
    include_bytes!("../../tests/fixtures/v2.sender-public")
  } else {
    SIGNER.public_key().as_ref()
  };
  let _ = decrypt_bytes(
    Zeroizing::new(data.to_vec()),
    &PRIVATE,
    Verification::Trusted(verifier),
  );
  let _ = decrypt_bytes(
    Zeroizing::new(data.to_vec()),
    &PRIVATE,
    Verification::AllowUnsignedLegacy,
  );
});

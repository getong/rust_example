//! A regression guard against replacing the full-ciphertext signature with header || GCM tag.
use aws_lc_rs::{
  aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey},
  cipher::{AES_256, EncryptingKey, UnboundCipherKey},
  signature::{KeyPair, ML_DSA_87, ML_DSA_87_SIGNING, PqdsaKeyPair, UnparsedPublicKey},
};

mod common;

#[test]
fn recipient_can_change_ciphertext_while_preserving_gcm_tag_and_tag_only_signature() {
  // Public test material. A legitimate KEM recipient knows this symmetric key.
  let raw_key = [7u8; 32];
  let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &raw_key).unwrap());
  let nonce = [9u8; 12];
  let header = b"fixed authenticated header, plaintext length 32";
  let original_plaintext = [42u8; 32];
  let mut original = original_plaintext.to_vec();
  key
    .seal_in_place_append_tag(
      Nonce::assume_unique_for_key(nonce),
      Aad::from(&header[..]),
      &mut original,
    )
    .unwrap();

  // GCM H = AES_K(0^128). ECB is used only for this one test block, never for file encryption.
  let mut h = [0u8; 16];
  EncryptingKey::ecb(UnboundCipherKey::new(&AES_256, &raw_key).unwrap())
    .unwrap()
    .encrypt(&mut h)
    .unwrap();
  let mut changed = original.clone();
  // For two adjacent GHASH blocks, deltas 1 and H cancel:
  // (C1 + 1) H^3 + (C2 + H) H^2 = C1 H^3 + C2 H^2.
  // GCM's field identity is represented by the high bit in the first byte.
  changed[0] ^= 0x80;
  for (byte, delta) in changed[16 .. 32].iter_mut().zip(h) {
    *byte ^= delta;
  }
  assert_eq!(&changed[32 ..], &original[32 ..]);
  assert_ne!(&changed[.. 32], &original[.. 32]);

  let signer = PqdsaKeyPair::generate(&ML_DSA_87_SIGNING).unwrap();
  let mut tag_only = header.to_vec();
  tag_only.extend_from_slice(&original[32 ..]);
  let mut signature = vec![0; ML_DSA_87_SIGNING.signature_len()];
  signer.sign(&tag_only, &mut signature).unwrap();
  let mut forged_tag_only = header.to_vec();
  forged_tag_only.extend_from_slice(&changed[32 ..]);
  UnparsedPublicKey::new(&ML_DSA_87, signer.public_key())
    .verify(&forged_tag_only, &signature)
    .unwrap();
  // Full-ciphertext binding rejects the exact same forgery, even though AEAD accepts it.
  let mut full_signature = vec![0; ML_DSA_87_SIGNING.signature_len()];
  signer
    .sign(
      &common::v3_transcript(header, &original),
      &mut full_signature,
    )
    .unwrap();
  assert!(
    UnparsedPublicKey::new(&ML_DSA_87, signer.public_key())
      .verify(&common::v3_transcript(header, &changed), &full_signature)
      .is_err()
  );
  let forged_plaintext = key
    .open_in_place(
      Nonce::assume_unique_for_key(nonce),
      Aad::from(&header[..]),
      &mut changed,
    )
    .unwrap();
  assert_ne!(forged_plaintext, &original_plaintext);
}

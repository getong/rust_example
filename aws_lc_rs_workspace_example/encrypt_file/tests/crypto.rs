use aws_lc_rs::{
  kem::{Ciphertext, DecapsulationKey, ML_KEM_1024},
  signature::{KeyPair, ML_DSA_87_SIGNING, PqdsaKeyPair},
};
use encrypt_file::{
  keys::{parse_key, protect_key, unlock_key},
  signing::verify_envelope,
  *,
};
use zeroize::Zeroizing;

fn signer() -> PqdsaKeyPair {
  PqdsaKeyPair::generate(&ML_DSA_87_SIGNING).unwrap()
}
fn receiver() -> DecapsulationKey {
  DecapsulationKey::generate(&ML_KEM_1024).unwrap()
}

#[test]
fn suite_sizes_match_backend() {
  let key = receiver();
  let public = key.encapsulation_key().unwrap();
  assert_eq!(key.key_bytes().unwrap().as_ref().len(), PRIVATE_KEY_LEN);
  assert_eq!(public.key_bytes().unwrap().as_ref().len(), PUBLIC_KEY_LEN);
  assert_eq!(
    public.encapsulate().unwrap().0.as_ref().len(),
    KEM_CIPHERTEXT_LEN
  );
  let signing = signer();
  use aws_lc_rs::encoding::AsRawBytes;
  assert_eq!(signing.public_key().as_ref().len(), SIGN_PUBLIC_KEY_LEN);
  assert_eq!(
    signing.private_key().as_raw_bytes().unwrap().as_ref().len(),
    SIGN_PRIVATE_KEY_LEN
  );
  assert_eq!(ML_DSA_87_SIGNING.signature_len(), SIGNATURE_LEN);
  assert_eq!(HEADER_LEN, 1694);
}

#[test]
fn signed_roundtrip_and_tampering_never_return_plaintext() {
  let receiver = receiver();
  let signer = signer();
  let public = receiver.encapsulation_key().unwrap().key_bytes().unwrap();
  let verifier = Verification::Trusted(signer.public_key().as_ref());
  for plain in [vec![], b"signed content\n".to_vec(), vec![42; 65537]] {
    let sealed = encrypt_bytes(Zeroizing::new(plain.clone()), public.as_ref(), &signer).unwrap();
    let bytes = sealed.to_bytes();
    assert_eq!(
      bytes.len(),
      HEADER_LEN + plain.len() + TAG_LEN + SIGNATURE_LEN
    );
    assert_eq!(
      *decrypt_bytes(Zeroizing::new(bytes.clone()), &receiver, verifier).unwrap(),
      plain
    );
    for index in [
      0,
      8,
      9,
      10,
      SALT_END,
      KEM_END,
      LEGACY_HEADER_LEN,
      SENDER_END,
      RECIPIENT_END,
      HEADER_LEN,
      bytes.len() - 1,
    ] {
      let mut bad = bytes.clone();
      bad[index] ^= 1;
      assert!(decrypt_bytes(Zeroizing::new(bad), &receiver, verifier).is_err());
    }
    for len in [0, 9, HEADER_LEN - 1, HEADER_LEN + TAG_LEN, bytes.len() - 1] {
      assert!(decrypt_bytes(Zeroizing::new(bytes[.. len].to_vec()), &receiver, verifier).is_err());
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(decrypt_bytes(Zeroizing::new(extra), &receiver, verifier).is_err());
    assert!(matches!(
      decrypt_bytes(
        Zeroizing::new(bytes),
        &receiver,
        Verification::AllowUnsignedLegacy
      ),
      Err(Error::SignatureRequired)
    ));
  }
}

#[test]
fn replacement_wrong_recipient_and_downgrade_are_rejected() {
  let private = receiver();
  let public = private.encapsulation_key().unwrap().key_bytes().unwrap();
  let trusted = signer();
  let attacker = signer();
  let unrelated = receiver();
  let trusted_verifier = Verification::Trusted(trusted.public_key().as_ref());
  let replacement = encrypt_bytes(
    Zeroizing::new(b"attacker content".to_vec()),
    public.as_ref(),
    &attacker,
  )
  .unwrap()
  .to_bytes();
  assert!(matches!(
    verify_envelope(&replacement, trusted_verifier),
    Err(Error::SignatureInvalid)
  ));
  let good = encrypt_bytes(
    Zeroizing::new(b"content".to_vec()),
    public.as_ref(),
    &trusted,
  )
  .unwrap()
  .to_bytes();
  assert!(matches!(
    decrypt_bytes(Zeroizing::new(good.clone()), &unrelated, trusted_verifier),
    Err(Error::AuthenticationFailed)
  ));
  // A signature copied from another file cannot authenticate an otherwise valid ciphertext.
  let mut replaced = replacement;
  let end = replaced.len();
  let good_end = good.len();
  replaced[end - SIGNATURE_LEN ..].copy_from_slice(&good[good_end - SIGNATURE_LEN ..]);
  assert!(verify_envelope(&replaced, trusted_verifier).is_err());
  let mut downgraded = good;
  downgraded[8] = 1;
  assert!(matches!(
    verify_envelope(&downgraded, trusted_verifier),
    Err(Error::SignatureRequired)
  ));
}

#[test]
fn fixed_v1_fixture_requires_explicit_legacy_policy() {
  let private = DecapsulationKey::new(&ML_KEM_1024, include_bytes!("fixtures/v1.private")).unwrap();
  let bytes = include_bytes!("fixtures/v1.encrypted");
  let signer = signer();
  assert!(matches!(
    verify_envelope(bytes, Verification::Trusted(signer.public_key().as_ref())),
    Err(Error::SignatureRequired)
  ));
  assert_eq!(
    &*decrypt_bytes(
      Zeroizing::new(bytes.to_vec()),
      &private,
      Verification::AllowUnsignedLegacy
    )
    .unwrap(),
    include_bytes!("fixtures/v1.plaintext")
  );
  for index in [
    0,
    8,
    9,
    10,
    SALT_END,
    KEM_END,
    LEGACY_HEADER_LEN,
    bytes.len() - 1,
  ] {
    let mut bad = bytes.to_vec();
    bad[index] ^= 1;
    assert!(
      decrypt_bytes(
        Zeroizing::new(bad),
        &private,
        Verification::AllowUnsignedLegacy
      )
      .is_err()
    );
  }
}

#[test]
fn protected_keys_and_structured_errors() {
  let private = receiver();
  let raw = private.key_bytes().unwrap();
  assert!(matches!(
    protect_key(raw.as_ref(), KeyKind::KemPrivate, Protection::Password("")),
    Err(Error::PasswordRequired)
  ));
  let protected = protect_key(
    raw.as_ref(),
    KeyKind::KemPrivate,
    Protection::Password(" 密码 "),
  )
  .unwrap();
  assert!(matches!(
    unlock_key(protected.clone(), KeyKind::KemPrivate, "bad"),
    Err(Error::KeyUnlockFailed)
  ));
  assert_eq!(
    unlock_key(protected.clone(), KeyKind::KemPrivate, " 密码 ")
      .unwrap()
      .as_slice(),
    raw.as_ref()
  );
  assert!(matches!(
    parse_key(&protected, KeyKind::KemPublic),
    Err(Error::Corrupt)
  ));
  let mut bad = protected.clone();
  bad[11 .. 15].copy_from_slice(&u32::MAX.to_le_bytes());
  assert!(matches!(
    parse_key(&bad, KeyKind::KemPrivate),
    Err(Error::KdfParameters)
  ));
  assert!(matches!(
    unlock_key(bad, KeyKind::KemPrivate, " 密码 "),
    Err(Error::KdfParameters)
  ));
  let mut bad = protected;
  let end = bad.len();
  bad[end - 1] ^= 1;
  assert!(matches!(
    unlock_key(bad, KeyKind::KemPrivate, " 密码 "),
    Err(Error::KeyUnlockFailed)
  ));
}

#[test]
fn legacy_password_key_keeps_fixed_pbkdf2_contract() {
  use aws_lc_rs::{
    aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey},
    pbkdf2,
  };
  let mut header = b"ALCFKEY\0\x01\x01\x01".to_vec();
  header.extend_from_slice(&[7; 32]);
  header.extend_from_slice(&[8; 12]);
  let mut derived = Zeroizing::new([0u8; 32]);
  pbkdf2::derive(
    pbkdf2::PBKDF2_HMAC_SHA256,
    std::num::NonZeroU32::new(600000).unwrap(),
    &[7; 32],
    b"legacy",
    &mut *derived,
  );
  let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &*derived).unwrap());
  let raw = include_bytes!("fixtures/v1.private");
  let mut body = raw.to_vec();
  key
    .seal_in_place_append_tag(
      Nonce::assume_unique_for_key([8; 12]),
      Aad::from(&header),
      &mut body,
    )
    .unwrap();
  header.extend_from_slice(&body);
  assert_eq!(
    unlock_key(Zeroizing::new(header), KeyKind::KemPrivate, "legacy")
      .unwrap()
      .as_slice(),
    raw
  );
}

#[test]
fn nist_fips203_known_answers() {
  for (private, public, ciphertext, expected) in [
    (
      &include_bytes!("fixtures/acvp-valid.dk")[..],
      &include_bytes!("fixtures/acvp-valid.ek")[..],
      &include_bytes!("fixtures/acvp-valid.c")[..],
      &include_bytes!("fixtures/acvp-valid.k")[..],
    ),
    (
      &include_bytes!("fixtures/acvp-reject.dk")[..],
      &include_bytes!("fixtures/acvp-reject.ek")[..],
      &include_bytes!("fixtures/acvp-reject.c")[..],
      &include_bytes!("fixtures/acvp-reject.k")[..],
    ),
  ] {
    let key = DecapsulationKey::new(&ML_KEM_1024, private).unwrap();
    let encapsulation = aws_lc_rs::kem::EncapsulationKey::new(&ML_KEM_1024, public).unwrap();
    assert_eq!(encapsulation.key_bytes().unwrap().as_ref(), public);
    assert_eq!(
      key
        .decapsulate(Ciphertext::from(ciphertext))
        .unwrap()
        .as_ref(),
      expected
    );
  }
}

#[test]
fn valid_signature_does_not_bypass_aead_authentication() {
  use aws_lc_rs::digest::{SHA512, digest};
  let private = receiver();
  let signer = signer();
  let public = private.encapsulation_key().unwrap().key_bytes().unwrap();
  let mut sealed = encrypt_bytes(
    Zeroizing::new(b"original".to_vec()),
    public.as_ref(),
    &signer,
  )
  .unwrap();
  sealed.ciphertext[0] ^= 1;
  // Build the documented transcript independently: valid signer, invalid AEAD ciphertext.
  let mut transcript = b"encrypt_file/v2/signature/ML-DSA-87/SHA-512\0".to_vec();
  transcript.extend_from_slice(&sealed.header);
  transcript.extend_from_slice(digest(&SHA512, &sealed.ciphertext).as_ref());
  signer.sign(&transcript, &mut sealed.signature).unwrap();
  let bytes = sealed.to_bytes();
  let verification = Verification::Trusted(signer.public_key().as_ref());
  verify_envelope(&bytes, verification).unwrap();
  assert!(matches!(
    decrypt_bytes(Zeroizing::new(bytes), &private, verification),
    Err(Error::AuthenticationFailed)
  ));
}

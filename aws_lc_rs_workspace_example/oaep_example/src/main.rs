use std::{collections::HashMap, error::Error, fmt::Write};

use aws_lc_rs::{
  encoding::{AsDer, Pkcs8V1Der, PublicKeyX509Der},
  rsa::{
    KeyPair, KeySize, OAEP_SHA256_MGF1SHA256, OaepPrivateDecryptingKey, OaepPublicEncryptingKey,
    PrivateDecryptingKey, PublicEncryptingKey,
  },
  signature::KeyPair as _,
};
use sha2::{Digest, Sha256};

const OAEP_LABEL: &[u8] = b"oaep-example:client-password:v1";

#[derive(Debug, Clone)]
struct KeyMaterial {
  kid: String,
  private_key_der: Vec<u8>,
  public_key_der: Vec<u8>,
}

#[derive(Debug)]
struct EncryptedPackage {
  kid: String,
  algorithm: &'static str,
  ciphertext: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyStatus {
  Active,
  DecryptOnly,
  Revoked,
}

#[derive(Debug, Clone)]
struct StoredKey {
  status: KeyStatus,
  private_key_der: Vec<u8>,
  public_key_der: Vec<u8>,
}

#[derive(Default)]
struct ServerKeyStore {
  keys_by_kid: HashMap<String, StoredKey>,
}

impl ServerKeyStore {
  fn insert(&mut self, key_material: KeyMaterial, status: KeyStatus) {
    self.keys_by_kid.insert(
      key_material.kid,
      StoredKey {
        status,
        private_key_der: key_material.private_key_der,
        public_key_der: key_material.public_key_der,
      },
    );
  }

  fn decrypt_client_password(
    &self,
    encrypted_package: &EncryptedPackage,
  ) -> Result<Vec<u8>, Box<dyn Error>> {
    let stored_key = self
      .keys_by_kid
      .get(&encrypted_package.kid)
      .ok_or_else(|| format!("unknown kid: {}", encrypted_package.kid))?;

    if stored_key.status == KeyStatus::Revoked {
      return Err(format!("kid is revoked: {}", encrypted_package.kid).into());
    }

    decrypt_client_password(&stored_key.private_key_der, encrypted_package)
  }

  fn active_public_key(&self) -> Result<KeyMaterial, Box<dyn Error>> {
    let (kid, stored_key) = self
      .keys_by_kid
      .iter()
      .find(|(_, stored_key)| stored_key.status == KeyStatus::Active)
      .ok_or("no active key available")?;

    Ok(KeyMaterial {
      kid: kid.clone(),
      private_key_der: stored_key.private_key_der.clone(),
      public_key_der: stored_key.public_key_der.clone(),
    })
  }

  fn set_status(&mut self, kid: &str, status: KeyStatus) -> Result<(), Box<dyn Error>> {
    let stored_key = self
      .keys_by_kid
      .get_mut(kid)
      .ok_or_else(|| format!("unknown kid: {kid}"))?;

    stored_key.status = status;
    Ok(())
  }

  fn active_kid(&self) -> Option<&str> {
    self
      .keys_by_kid
      .iter()
      .find(|(_, stored_key)| stored_key.status == KeyStatus::Active)
      .map(|(kid, _)| kid.as_str())
  }

  fn count_by_status(&self, status: KeyStatus) -> usize {
    self
      .keys_by_kid
      .values()
      .filter(|stored_key| stored_key.status == status)
      .count()
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let password = b"correct horse battery staple";

  let previous_key = generate_rsa_key_material()?;
  let current_key = generate_rsa_key_material()?;
  let next_key = generate_rsa_key_material()?;

  let mut server_key_store = ServerKeyStore::default();
  server_key_store.insert(previous_key.clone(), KeyStatus::DecryptOnly);
  server_key_store.insert(current_key.clone(), KeyStatus::Active);
  server_key_store.insert(next_key.clone(), KeyStatus::Revoked);

  println!("initial active kid: {}", current_key.kid);
  println!("registered keys: {}", server_key_store.keys_by_kid.len());
  println!(
    "status counts => active: {}, decrypt_only: {}, revoked: {}",
    server_key_store.count_by_status(KeyStatus::Active),
    server_key_store.count_by_status(KeyStatus::DecryptOnly),
    server_key_store.count_by_status(KeyStatus::Revoked),
  );

  // Phase 1: a client encrypts with the currently active public key.
  let active_key = server_key_store.active_public_key()?;
  let package_for_current_key = encrypt_for_client(&active_key, password)?;
  let final_password = server_key_store.decrypt_client_password(&package_for_current_key)?;

  println!("phase 1 package kid: {}", package_for_current_key.kid);
  println!("password recovered: {} bytes", final_password.len());

  // Phase 2: rotate keys. The current active key becomes decrypt_only,
  // and the next key is promoted to active for new clients.
  server_key_store.set_status(&current_key.kid, KeyStatus::DecryptOnly)?;
  server_key_store.set_status(&next_key.kid, KeyStatus::Active)?;

  let rotated_active_kid = server_key_store
    .active_kid()
    .ok_or("no active kid after rotation")?;
  println!("active kid after rotation: {}", rotated_active_kid);

  let package_for_next_key = encrypt_for_client(&next_key, password)?;
  let next_password = server_key_store.decrypt_client_password(&package_for_next_key)?;
  println!("phase 2 package kid: {}", package_for_next_key.kid);
  println!("rotated key recovered: {} bytes", next_password.len());

  // Old ciphertext still decrypts because the key is now decrypt_only.
  let old_password = server_key_store.decrypt_client_password(&package_for_current_key)?;
  println!(
    "old package still decrypts after rotation: {} bytes",
    old_password.len()
  );

  // Phase 3: retire the previous key completely.
  server_key_store.set_status(&current_key.kid, KeyStatus::Revoked)?;
  let revoked_result = server_key_store.decrypt_client_password(&package_for_current_key);
  println!(
    "old package after revocation: {}",
    match revoked_result {
      Ok(_) => "unexpected success".to_string(),
      Err(err) => err.to_string(),
    }
  );

  Ok(())
}

fn generate_rsa_key_material() -> Result<KeyMaterial, Box<dyn Error>> {
  // 1. Generate a 2048-bit RSA key pair.
  let key_pair = KeyPair::generate(KeySize::Rsa2048)?;

  // 2. Export the public key as X.509 SubjectPublicKeyInfo DER.
  let public_key_der: PublicKeyX509Der<'static> = key_pair.public_key().as_der()?;

  // aws-lc-rs 1.16.3 does not provide `PrivateDecryptingKey::from_key_pair`,
  // so we export the private key as PKCS#8 and load it back via the
  // encryption-focused API.
  let private_key_der: Pkcs8V1Der<'static> = key_pair.as_der()?;

  let public_key_der = public_key_der.as_ref().to_vec();

  Ok(KeyMaterial {
    kid: compute_kid(&public_key_der),
    private_key_der: private_key_der.as_ref().to_vec(),
    public_key_der,
  })
}

fn encrypt_for_client(
  key_material: &KeyMaterial,
  plaintext: &[u8],
) -> Result<EncryptedPackage, Box<dyn Error>> {
  let public_key = PublicEncryptingKey::from_der(&key_material.public_key_der)?;
  let public_key = OaepPublicEncryptingKey::new(public_key)?;

  if plaintext.len() > public_key.max_plaintext_size(&OAEP_SHA256_MGF1SHA256) {
    return Err("plaintext too large for RSA-OAEP with this key size".into());
  }

  let mut ciphertext = vec![0u8; public_key.ciphertext_size()];
  let ciphertext = public_key.encrypt(
    &OAEP_SHA256_MGF1SHA256,
    plaintext,
    &mut ciphertext,
    Some(OAEP_LABEL),
  )?;

  Ok(EncryptedPackage {
    kid: key_material.kid.clone(),
    algorithm: "RSA-OAEP-256",
    ciphertext: ciphertext.to_vec(),
  })
}

fn decrypt_client_password(
  private_key_der: &[u8],
  encrypted_package: &EncryptedPackage,
) -> Result<Vec<u8>, Box<dyn Error>> {
  if encrypted_package.algorithm != "RSA-OAEP-256" {
    return Err(format!("unsupported algorithm: {}", encrypted_package.algorithm).into());
  }

  let private_key = PrivateDecryptingKey::from_pkcs8(private_key_der)?;
  let private_key = OaepPrivateDecryptingKey::new(private_key)?;

  let mut plaintext = vec![0u8; private_key.min_output_size()];
  let plaintext = private_key.decrypt(
    &OAEP_SHA256_MGF1SHA256,
    &encrypted_package.ciphertext,
    &mut plaintext,
    Some(OAEP_LABEL),
  )?;

  Ok(plaintext.to_vec())
}

fn compute_kid(public_key_der: &[u8]) -> String {
  let digest = Sha256::digest(public_key_der);
  let mut kid = String::with_capacity(digest.len() * 2);

  for byte in digest {
    let _ = write!(&mut kid, "{byte:02x}");
  }

  kid
}

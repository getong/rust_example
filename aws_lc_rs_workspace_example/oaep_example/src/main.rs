use std::error::Error;

use aws_lc_rs::{
  encoding::{AsDer, Pkcs8V1Der, PublicKeyX509Der},
  rsa::{
    KeyPair, KeySize, OAEP_SHA256_MGF1SHA256, OaepPrivateDecryptingKey, OaepPublicEncryptingKey,
    PrivateDecryptingKey, PublicEncryptingKey,
  },
  signature::KeyPair as _,
};

fn main() -> Result<(), Box<dyn Error>> {
  let password = b"correct horse battery staple";

  let (private_key_der, public_key_der) = generate_rsa_key_material()?;
  println!("public key der bytes: {}", public_key_der.len());

  // Demo data standing in for bytes a client would send to the server.
  let encrypted_data = encrypt_for_client(&public_key_der, password)?;
  let final_password = decrypt_client_password(&private_key_der, &encrypted_data)?;

  println!(
    "decrypted password: {}",
    String::from_utf8_lossy(&final_password)
  );

  Ok(())
}

fn generate_rsa_key_material() -> Result<(Vec<u8>, Vec<u8>), Box<dyn Error>> {
  // 1. Generate a 2048-bit RSA key pair.
  let key_pair = KeyPair::generate(KeySize::Rsa2048)?;

  // 2. Export the public key as X.509 SubjectPublicKeyInfo DER.
  let public_key_der: PublicKeyX509Der<'static> = key_pair.public_key().as_der()?;

  // aws-lc-rs 1.16.3 does not provide `PrivateDecryptingKey::from_key_pair`,
  // so we export the private key as PKCS#8 and load it back via the
  // encryption-focused API.
  let private_key_der: Pkcs8V1Der<'static> = key_pair.as_der()?;

  Ok((
    private_key_der.as_ref().to_vec(),
    public_key_der.as_ref().to_vec(),
  ))
}

fn encrypt_for_client(public_key_der: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
  let public_key = PublicEncryptingKey::from_der(public_key_der)?;
  let public_key = OaepPublicEncryptingKey::new(public_key)?;

  if plaintext.len() > public_key.max_plaintext_size(&OAEP_SHA256_MGF1SHA256) {
    return Err("plaintext too large for RSA-OAEP with this key size".into());
  }

  let mut ciphertext = vec![0u8; public_key.ciphertext_size()];
  let ciphertext = public_key.encrypt(&OAEP_SHA256_MGF1SHA256, plaintext, &mut ciphertext, None)?;

  Ok(ciphertext.to_vec())
}

fn decrypt_client_password(
  private_key_der: &[u8],
  encrypted_data: &[u8],
) -> Result<Vec<u8>, Box<dyn Error>> {
  let private_key = PrivateDecryptingKey::from_pkcs8(private_key_der)?;
  let private_key = OaepPrivateDecryptingKey::new(private_key)?;

  let mut plaintext = vec![0u8; private_key.min_output_size()];
  let plaintext = private_key.decrypt(
    &OAEP_SHA256_MGF1SHA256,
    encrypted_data,
    &mut plaintext,
    None,
  )?;

  Ok(plaintext.to_vec())
}

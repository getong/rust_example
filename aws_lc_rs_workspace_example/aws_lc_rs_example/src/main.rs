use aws_lc_rs::{
  aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
  digest::{SHA256, digest},
  hmac,
  rand::{self, SecureRandom},
};

fn main() {
  println!("=== AWS-LC-RS example ===\n");

  example_sha256();

  example_hmac();

  example_random();

  example_aes_encryption();
}

fn example_sha256() {
  println!("1. SHA-256 hash example");
  println!("   ---------------");

  let data = b"Hello, aws-lc-rs!";
  let hash = digest(&SHA256, data);

  println!("   original data: {:?}", std::str::from_utf8(data).unwrap());
  println!("   SHA-256:  {}", hex_encode(hash.as_ref()));
  println!();
}

fn example_hmac() {
  println!("2. HMAC example");
  println!("   ---------");

  let rng = rand::SystemRandom::new();
  let key_value: [u8; 32] = generate_random_bytes(&rng);

  let key = hmac::Key::new(hmac::HMAC_SHA256, &key_value);
  let message = b"Important message";

  // HMAC
  let tag = hmac::sign(&key, message);

  println!("   msg:     {:?}", std::str::from_utf8(message).unwrap());
  println!("   key:     {}", hex_encode(&key_value));
  println!("   HMAC:     {}", hex_encode(tag.as_ref()));

  // verify HMAC
  let verification_key = hmac::Key::new(hmac::HMAC_SHA256, &key_value);
  match hmac::verify(&verification_key, message, tag.as_ref()) {
    Ok(_) => println!("   verify:     pass! ✓"),
    Err(_) => println!("   verify:     failed! ✗"),
  }
  println!();
}

fn example_random() {
  println!("3. random number example");
  println!("   --------------");

  let rng = rand::SystemRandom::new();

  // generate 16 bytes random number
  let random_bytes: [u8; 16] = generate_random_bytes(&rng);
  println!("   16 bytes example: {}", hex_encode(&random_bytes));

  // generate 32 bytes random number
  let random_bytes: [u8; 32] = generate_random_bytes(&rng);
  println!("   32 bytes number: {}", hex_encode(&random_bytes));
  println!();
}

fn example_aes_encryption() {
  println!("4. AES-256-GCM example");
  println!("   -------------------------");

  let rng = rand::SystemRandom::new();

  // generate key
  let key_bytes: [u8; 32] = generate_random_bytes(&rng);
  let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes).unwrap();
  let key = LessSafeKey::new(unbound_key);

  // generate nonce
  let nonce_bytes: [u8; NONCE_LEN] = generate_random_bytes(&rng);
  let nonce = Nonce::assume_unique_for_key(nonce_bytes);

  // encrypted data
  let plaintext = b"This is a secret message!";
  let mut in_out = plaintext.to_vec();

  println!(
    "   original data: {:?}",
    std::str::from_utf8(plaintext).unwrap()
  );
  println!("   key:     {}", hex_encode(&key_bytes));
  println!("   Nonce:    {}", hex_encode(&nonce_bytes));

  // cipher
  let aad = Aad::empty(); // Additional Authenticated Data
  key
    .seal_in_place_append_tag(nonce, aad, &mut in_out)
    .unwrap();

  println!("   cipher:     {}", hex_encode(&in_out));

  // decrypt
  let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes).unwrap();
  let key = LessSafeKey::new(unbound_key);
  let nonce = Nonce::assume_unique_for_key(nonce_bytes);
  let aad = Aad::empty();

  let decrypted = key.open_in_place(nonce, aad, &mut in_out).unwrap();

  println!(
    "   decrypt data: {:?}",
    std::str::from_utf8(decrypted).unwrap()
  );
  println!();
}

/// helper
fn generate_random_bytes<const N: usize>(rng: &dyn SecureRandom) -> [u8; N] {
  let mut bytes = [0u8; N];
  rng.fill(&mut bytes).unwrap();
  bytes
}

/// helper
fn hex_encode(bytes: &[u8]) -> String {
  bytes
    .iter()
    .map(|b| format!("{:02x}", b))
    .collect::<String>()
}

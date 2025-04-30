use k256::{
  SecretKey,
  ecdsa::{
    Signature, SigningKey, VerifyingKey,
    signature::{Signer, Verifier},
  },
  elliptic_curve::rand_core::OsRng as K256OsRng,
};

fn main() {
  // Generate a new random private key using the OsRng from k256's dependencies
  let signing_key = SigningKey::random(&mut K256OsRng);
  let verifying_key = VerifyingKey::from(&signing_key);

  println!("Generated K256 Keypair:");
  println!("Private key: {}", hex::encode(signing_key.to_bytes()));
  println!(
    "Public key: {}",
    hex::encode(verifying_key.to_encoded_point(false).as_bytes())
  );

  // Sign a message
  let message = b"Hello, K256 signing!";
  let signature: Signature = signing_key.sign(message);
  println!("\nSigned message: \"{}\"", String::from_utf8_lossy(message));
  println!("Signature (DER): {}", hex::encode(signature.to_der()));

  // Verify the signature
  let result = verifying_key.verify(message, &signature);
  println!(
    "\nSignature verification: {}",
    if result.is_ok() { "SUCCESS" } else { "FAILED" }
  );

  // Example of manually working with keys
  println!("\nAlternative API example:");
  // Create a secret key using the OsRng from k256's dependencies
  let secret_key = SecretKey::random(&mut K256OsRng);
  // Use the secret key's public key method
  let public_key = secret_key.public_key();

  println!("Secret key: {}", hex::encode(secret_key.to_bytes()));
  println!("Public key: {}", hex::encode(public_key.to_sec1_bytes()));
}

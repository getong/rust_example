use libp2p_identity::ed25519::{Keypair, PublicKey, SecretKey};
// use rand::Rng;

fn main() {
  // Generate a new keypair
  // let mut rng = rand::thread_rng();
  let keypair = Keypair::generate();

  // Get the public key and secret key from the keypair
  let public_key: PublicKey = keypair.public();
  let _secret_key: SecretKey = keypair.secret();

  // Serialize the public key to bytes
  let public_key_bytes = public_key.to_bytes();

  // Deserialize the public key from bytes
  let decoded_public_key = PublicKey::try_from_bytes(&public_key_bytes).unwrap();

  // Print the original and decoded public keys to verify they match
  println!("Original public key: {:?}", public_key);
  println!("Decoded public key: {:?}", public_key_bytes);
  assert_eq!(public_key, decoded_public_key);

  test_verify_function();
}

fn test_verify_function() {
  // Generate a new keypair
  let keypair = Keypair::generate();

  // Get the public key from the keypair
  let public_key = keypair.public();

  // Create some data to sign
  let data = b"This is some data to sign";

  // Sign the data
  let signature = keypair.sign(data);

  // Verify the signature
  let verified = public_key.verify(data, &signature);

  // Print whether the signature was verified
  println!("{}", verified);
}

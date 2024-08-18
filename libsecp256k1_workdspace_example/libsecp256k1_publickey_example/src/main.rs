use libsecp256k1::{PublicKey, SecretKey};

fn main() {
  // Generate a new random secret key
  let secret_key = SecretKey::random(&mut rand::thread_rng());

  // Derive the public key from the secret key
  let public_key = PublicKey::from_secret_key(&secret_key);

  // Serialize the public key in uncompressed format (65 bytes, with 0x04 prefix)
  let serialized_uncompressed = public_key.serialize();

  // Serialize the public key in compressed format (33 bytes, with 0x02 or 0x03 prefix)
  let serialized_compressed = public_key.serialize_compressed();

  // Print the public key in hexadecimal format (uncompressed)
  println!(
    "Public Key (Uncompressed): 0x{}",
    hex::encode(serialized_uncompressed)
  );

  // Print the public key in hexadecimal format (compressed)
  println!(
    "Public Key (Compressed): 0x{}",
    hex::encode(serialized_compressed)
  );
}

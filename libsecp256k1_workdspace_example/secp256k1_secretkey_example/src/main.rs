use secp256k1::SecretKey;
use std::fs::File;
use std::io::Write;
use rand::thread_rng;

fn main() {
  // Generate a new secp256k1 secret key
  let secret_key = SecretKey::new(&mut thread_rng());

  // Convert the secret key to a 32-byte array
  let secret_key_bytes = secret_key.secret_bytes();

  // Encode the byte array to a hex string
  let hex_key = hex::encode(secret_key_bytes);

  // Write the hex key to identity.txt
  let mut file = File::create("identity.txt").expect("Unable to create file");
  file.write_all(hex_key.as_bytes()).expect("Unable to write data");

  println!("Identity file created: identity.txt");
}

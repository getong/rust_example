use libsecp256k1::SecretKey;
use std::fs::File;
use std::io::Write;

fn main() {
  // Generate a new secp256k1 secret key
  let secret_key = SecretKey::random(&mut rand::thread_rng());

  // Convert the secret key to hex
  let hex_key = hex::encode(secret_key.serialize());

  // Write the hex key to identity.txt
  let mut file = File::create("identity.txt").expect("Unable to create file");
  file
    .write_all(hex_key.as_bytes())
    .expect("Unable to write data");

  println!("Identity file created: identity.txt");
}

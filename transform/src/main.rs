use pad::PadStr;
use recrypt::api::Plaintext;
use recrypt::prelude::*;
use std::env;

fn unsize<T>(x: &[T]) -> &[T] {
  x
}

fn main() {
  // create a new recrypt
  let recrypt = Recrypt::new();

  let mut mystr = "Hello";

  let args: Vec<String> = env::args().collect();

  if args.len() > 1 {
    mystr = &args[1];
  }

  let x = Plaintext::new_from_slice(mystr.pad_to_width_with_char(384, ' ').as_bytes());

  let pt = x.unwrap();

  let signing_keypair = recrypt.generate_ed25519_key_pair();

  let (initial_priv_key, initial_pub_key) = recrypt.generate_key_pair().unwrap();

  let encrypted_val = recrypt
    .encrypt(&pt, &initial_pub_key, &signing_keypair)
    .unwrap();

  let (target_priv_key, target_pub_key) = recrypt.generate_key_pair().unwrap();

  let initial_to_target_transform_key = recrypt
    .generate_transform_key(&initial_priv_key, &target_pub_key, &signing_keypair)
    .unwrap();

  let transformed_val = recrypt
    .transform(
      encrypted_val,
      initial_to_target_transform_key,
      &signing_keypair,
    )
    .unwrap();

  let decrypted_val = recrypt.decrypt(transformed_val, &target_priv_key).unwrap();

  println!("\nInput string:\t{} ", mystr);
  println!(
    "\nSigning key:\t{} ",
    hex::encode(unsize(signing_keypair.bytes()))
  );
  println!(
    "\nInitial Private key:\t{} ",
    hex::encode(unsize(initial_priv_key.bytes()))
  );
  let (x, y) = initial_pub_key.bytes_x_y();
  println!(
    "\nInitial Public key:\t{},{} ",
    hex::encode(unsize(x)),
    hex::encode(unsize(y))
  );
  println!(
    "\nTarget Private key:\t{} ",
    hex::encode(unsize(target_priv_key.bytes()))
  );
  let (x, y) = target_pub_key.bytes_x_y();
  println!(
    "\nTarget Public key:\t{},{} ",
    hex::encode(unsize(x)),
    hex::encode(unsize(y))
  );

  println!(
    "\nDecrypted:\t{} ",
    String::from_utf8_lossy(decrypted_val.bytes())
  );
}

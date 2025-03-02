use rand_core::{OsRng, RngCore};

fn main() {
  // println!("Hello, world!");

  let mut key = [0u8; 16];
  OsRng.fill_bytes(&mut key);
  let random_u64 = OsRng.next_u64();

  println!("key:{:?}, random_u64:{}", key, random_u64);
}

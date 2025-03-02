use rand_core::{OsRng, TryRngCore};

fn main() {
  let mut key = [0u8; 16];

  OsRng.try_fill_bytes(&mut key).unwrap();
  let random_u64 = OsRng.try_next_u64().unwrap();
  println!("key:{:?}, random_u64:{}", key, random_u64);
}

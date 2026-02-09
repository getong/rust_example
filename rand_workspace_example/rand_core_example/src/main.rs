use getrandom::SysRng;
use rand_core::TryRng;

fn main() {
  let mut key = [0u8; 16];

  let mut rng = SysRng;
  rng.try_fill_bytes(&mut key).unwrap();
  let random_u64 = rng.try_next_u64().unwrap();
  println!("key:{:?}, random_u64:{}", key, random_u64);
}

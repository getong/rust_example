use std::{
  collections::hash_map::DefaultHasher,
  hash::{Hash, Hasher},
};

fn hash(value: &str) -> u64 {
  let mut s = DefaultHasher::new();

  value.hash(&mut s);
  s.finish()
}

struct BloomFilter<const N: usize> {
  bytes: [u8; N],
}

impl<const N: usize> BloomFilter<N> {
  fn new() -> Self {
    Self { bytes: [0; N] }
  }

  fn get_positions(&self, key: &str) -> (usize, u64) {
    let bit_size = N * 8;
    let pos = hash(key) % (bit_size as u64);

    ((pos / 8) as usize, pos % 8)
  }

  fn add(&mut self, key: &str) {
    let (byte_idx, bit_idx) = self.get_positions(key);

    self.bytes[byte_idx] |= 1 << bit_idx;
  }

  fn contains(&self, key: &str) -> bool {
    let (byte_idx, bit_idx) = self.get_positions(key);

    self.bytes[byte_idx] & (1 << bit_idx) != 0
  }
}

fn main() {
  let mut filter = BloomFilter::<10>::new();

  filter.add("test1");
  filter.add("test2");

  println!("test1: {}", filter.contains("test1"));
  println!("test2: {}", filter.contains("test2"));
  println!("test3: {}", filter.contains("test3"));
}

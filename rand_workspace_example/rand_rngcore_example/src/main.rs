use rand_core::{
  CryptoRng, RngCore,
  block::{BlockRng, BlockRngCore},
};

struct SeedRng([u32; 8]);

impl CryptoRng for SeedRng {}

impl RngCore for SeedRng {
  fn next_u32(&mut self) -> u32 {
    self.0.iter_mut().next().copied().unwrap()
  }

  fn next_u64(&mut self) -> u64 {
    self.0.iter_mut().next().copied().unwrap() as u64
  }

  fn fill_bytes(&mut self, dest: &mut [u8]) {
    for chunk in dest.chunks_mut(4) {
      chunk.copy_from_slice(&self.next_u32().to_be_bytes());
    }
  }
}

impl BlockRngCore for SeedRng {
  type Item = u32;
  type Results = [u32; 8];

  fn generate(&mut self, results: &mut Self::Results) {
    *results = self.0;
  }
}

// ✅ Define a new struct that wraps `BlockRng<SeedRng>`
struct CryptoBlockRng(BlockRng<SeedRng>);

// ✅ Implement `CryptoRng` for the wrapper struct
impl CryptoRng for CryptoBlockRng {}

// ✅ Implement `RngCore` by delegating to `BlockRng<SeedRng>`
impl RngCore for CryptoBlockRng {
  fn next_u32(&mut self) -> u32 {
    self.0.next_u32()
  }

  fn next_u64(&mut self) -> u64 {
    self.0.next_u64()
  }

  fn fill_bytes(&mut self, dest: &mut [u8]) {
    self.0.fill_bytes(dest);
  }
}

fn random(mut rng: impl RngCore + CryptoRng) {
  let mut bytes = [0u8; 8];
  rng.fill_bytes(&mut bytes);
  println!("bytes: {:?}", bytes);
}

fn main() {
  let backing = [0; 8];
  let rng = CryptoBlockRng(BlockRng::new(SeedRng(backing))); // Wrap BlockRng in CryptoBlockRng
  random(rng);
}

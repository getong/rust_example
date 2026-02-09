use std::convert::Infallible;

use rand_core::{Rng, TryRng};
use rand_core::block::{BlockRng, Generator};

#[derive(Clone, Debug)]
struct SeedGenerator {
  state: [u32; 8],
}

impl Generator for SeedGenerator {
  type Output = [u32; 8];

  fn generate(&mut self, output: &mut Self::Output) {
    // Extremely simple, deterministic output (not cryptographically secure).
    *output = self.state;
    self.state.rotate_left(1);
    self.state[0] = self.state[0].wrapping_add(0x9E37_79B9);
  }
}

// rand_core 0.10 moved the real APIs to `TryRng`/`Rng`.
// `RngCore` remains only as a deprecated stub trait.
#[derive(Clone, Debug)]
struct SeedRng(BlockRng<SeedGenerator>);

impl SeedRng {
  fn new(state: [u32; 8]) -> Self {
    Self(BlockRng::new(SeedGenerator { state }))
  }
}

impl TryRng for SeedRng {
  type Error = Infallible;

  fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
    Ok(self.0.next_word())
  }

  fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
    Ok(self.0.next_u64_from_u32())
  }

  fn try_fill_bytes(&mut self, bytes: &mut [u8]) -> Result<(), Self::Error> {
    self.0.fill_bytes(bytes);
    Ok(())
  }
}

fn random(mut rng: impl Rng) {
  let mut bytes = [0u8; 8];
  rng.fill_bytes(&mut bytes);
  println!("bytes: {:?}", bytes);
}

fn main() {
  let backing = [1, 2, 3, 4, 5, 6, 7, 8];
  let rng = SeedRng::new(backing);
  random(rng);
}

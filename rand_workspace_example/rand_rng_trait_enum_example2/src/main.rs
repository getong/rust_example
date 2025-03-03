use std::sync::Arc;

use rand::{self, Rng, RngCore, SeedableRng};
use rand_chacha::ChaChaRng;
use rand_xorshift::XorShiftRng;

#[derive(Clone, Debug)]
pub struct TestRng {
  rng: TestRngImpl,
}

#[derive(Clone, Debug)]
enum TestRngImpl {
  XorShift(XorShiftRng),
  ChaCha(ChaChaRng),
  PassThrough {
    off: usize,
    end: usize,
    data: Arc<[u8]>,
  },
  Recorder {
    rng: ChaChaRng,
    record: Vec<u8>,
  },
}

impl RngCore for TestRng {
  fn next_u32(&mut self) -> u32 {
    match &mut self.rng {
      TestRngImpl::XorShift(rng) => rng.next_u32(),
      TestRngImpl::ChaCha(rng) => rng.next_u32(),
      TestRngImpl::PassThrough { .. } => {
        let mut buf = [0; 4];
        self.fill_bytes(&mut buf[..]);
        u32::from_le_bytes(buf)
      }
      TestRngImpl::Recorder { rng, record } => {
        let read = rng.next_u32();
        record.extend_from_slice(&read.to_le_bytes());
        read
      }
    }
  }

  fn next_u64(&mut self) -> u64 {
    match &mut self.rng {
      TestRngImpl::XorShift(rng) => rng.next_u64(),
      TestRngImpl::ChaCha(rng) => rng.next_u64(),
      TestRngImpl::PassThrough { .. } => {
        let mut buf = [0; 8];
        self.fill_bytes(&mut buf[..]);
        u64::from_le_bytes(buf)
      }
      TestRngImpl::Recorder { rng, record } => {
        let read = rng.next_u64();
        record.extend_from_slice(&read.to_le_bytes());
        read
      }
    }
  }

  fn fill_bytes(&mut self, dest: &mut [u8]) {
    match &mut self.rng {
      TestRngImpl::XorShift(rng) => rng.fill_bytes(dest),
      TestRngImpl::ChaCha(rng) => rng.fill_bytes(dest),
      TestRngImpl::PassThrough { off, end, data } => {
        let bytes_to_copy = dest.len().min(*end - *off);
        dest[.. bytes_to_copy].copy_from_slice(&data[*off .. *off + bytes_to_copy]);
        *off += bytes_to_copy;
        for i in bytes_to_copy .. dest.len() {
          dest[i] = 0;
        }
      }
      TestRngImpl::Recorder { rng, record } => {
        rng.fill_bytes(dest);
        record.extend_from_slice(dest);
      }
    }
  }
}

impl rand_core::CryptoRng for TestRng {}

fn main() {
  let mut seed_rng = rand::rng();
  let chacha_rng = ChaChaRng::from_rng(&mut seed_rng);

  let mut rng = TestRng {
    rng: TestRngImpl::ChaCha(chacha_rng),
  };

  // Now, we can use Rng's `gen` method!
  let random_number: u32 = rng.random();
  println!("Random number: {}", random_number);

  let random_float: f64 = rng.random();
  println!("Random float: {}", random_float);

  let random_range: u32 = rng.random_range(10 .. 50);
  println!("Random number in range 10-50: {}", random_range);
}

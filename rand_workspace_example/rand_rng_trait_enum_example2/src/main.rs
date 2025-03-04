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

fn main() {
  chacha_rng_example();

  println!();
  xorshift_rng_example();

  println!();
  passthrough_rng_example();

  println!();
  record_rng_example();
}

fn chacha_rng_example() {
  let mut seed_rng = rand::rng();
  let chacha_rng = ChaChaRng::from_rng(&mut seed_rng);

  let mut rng = TestRng {
    rng: TestRngImpl::ChaCha(chacha_rng),
  };

  // Now, we can use Rng's `random` method!
  let random_number: u32 = rng.random();
  println!("chacha Random number: {}", random_number);

  let random_float: f64 = rng.random();
  println!("chacha Random float: {}", random_float);

  let random_range: u32 = rng.random_range(10 .. 50);
  println!("chacha Random number in range 10-50: {}", random_range);
}

fn xorshift_rng_example() {
  let mut seed_rng = rand::rng();
  let xorshift_rng = XorShiftRng::from_rng(&mut seed_rng);

  let mut rng = TestRng {
    rng: TestRngImpl::XorShift(xorshift_rng),
  };

  // Now, we can use Rng's `random` method!
  let random_number: u32 = rng.random();
  println!("xorshift Random number: {}", random_number);

  let random_float: f64 = rng.random();
  println!("xorshift Random float: {}", random_float);

  let random_range: u32 = rng.random_range(10 .. 50);
  println!("xorshift Random number in range 10-50: {}", random_range);
}

fn passthrough_rng_example() {
  let pass_through_data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10].into();
  let pass_through_rng = TestRngImpl::PassThrough {
    off: 0,
    end: 10,
    data: pass_through_data,
  };

  let mut rng = TestRng {
    rng: pass_through_rng,
  };

  // Now, we can use Rng's `random` method!
  let random_number: u32 = rng.random();
  println!("passthrough Random number: {}", random_number);

  let random_float: f64 = rng.random();
  println!("passthrough Random float: {}", random_float);

  let random_range: u32 = rng.random_range(10 .. 50);
  println!("passthrough Random number in range 10-50: {}", random_range);
}

fn record_rng_example() {
  let mut seed_rng = rand::rng();
  let chacha_rng = ChaChaRng::from_rng(&mut seed_rng);
  let record = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
  let record_rng = TestRngImpl::Recorder {
    rng: chacha_rng,
    record,
  };

  let mut rng = TestRng { rng: record_rng };

  // Now, we can use Rng's `random` method!
  let random_number: u32 = rng.random();
  println!("record Random number: {}", random_number);

  let random_float: f64 = rng.random();
  println!("record Random float: {}", random_float);

  let random_range: u32 = rng.random_range(10 .. 50);
  println!("record Random number in range 10-50: {}", random_range);
}

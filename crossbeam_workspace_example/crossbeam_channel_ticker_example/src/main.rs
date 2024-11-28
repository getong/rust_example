use std::time::{Duration, Instant};

use crossbeam_channel::tick;

fn simple_ticker() {
  let start = Instant::now();
  let ticker = tick(Duration::from_millis(100));

  for _ in 0 .. 5 {
    let msg = ticker.recv().unwrap();
    println!("{:?} elapsed: {:?}", msg, start.elapsed());
  }
}

fn main() {
  simple_ticker();
}

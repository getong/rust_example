use std::time::{Duration, Instant};

use crossbeam_channel::after;

fn simple_after() {
  let start = Instant::now();
  let af = after(Duration::from_millis(100));

  for _ in 0 .. 5 {
    af.recv().unwrap();
    println!("elapsed: {:?}", start.elapsed());
  }
}

fn main() {
  // println!("Hello, world!");
  simple_after();
}

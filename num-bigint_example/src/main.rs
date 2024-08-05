use num_bigint::BigUint;
use num_bigint::{RandBigInt, ToBigInt};
use num_traits::{One, Zero};
use std::mem::replace;

// Calculate large fibonacci numbers.
fn fib(n: usize) -> BigUint {
  let mut f0: BigUint = Zero::zero();
  let mut f1: BigUint = One::one();
  for _ in 0 .. n {
    let f2 = f0 + &f1;
    // This is a low cost way of swapping f0 with f1 and f1 with f2.
    f0 = replace(&mut f1, f2);
  }
  f0
}

fn main() {
  // println!("Hello, world!");
  // This is a very large number.
  println!("fib(1000) = {}", fib(1000));

  let mut rng = rand::thread_rng();
  let a = rng.gen_bigint(1000);

  let low = -10000.to_bigint().unwrap();
  let high = 10000.to_bigint().unwrap();
  let b = rng.gen_bigint_range(&low, &high);

  // Probably an even larger number.
  println!("{}", a * b);
}

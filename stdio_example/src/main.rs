// copy from https://medium.com/@wisdom-of-the-east/reading-a-list-from-stdin-in-rust-17cce3f9c1bc

use std::{fmt, io, str};

/// Read a single value from stdin
fn read<T>() -> T
where
  T: str::FromStr,
  <T as str::FromStr>::Err: fmt::Debug,
{
  let mut buf = String::new();
  io::stdin().read_line(&mut buf).unwrap();

  return buf.trim().parse::<T>().unwrap();
}

/// Read a list of values from stdin of unknown length
fn read_into_vec<T>() -> Vec<T>
where
  T: str::FromStr,
  <T as str::FromStr>::Err: fmt::Debug,
{
  let mut buf = String::new();
  io::stdin().read_line(&mut buf).unwrap();

  let mut x = vec![];
  for i in buf.split_whitespace() {
    x.push(i.trim().parse::<T>().unwrap());
  }

  return x;
}

/// Read a list of values from stdin of known length
fn read_into_vec_n<T>(n: usize) -> Vec<T>
where
  T: str::FromStr,
  <T as str::FromStr>::Err: fmt::Debug,
{
  let mut buf = String::new();
  io::stdin().read_line(&mut buf).unwrap();

  let mut x: Vec<T> = Vec::with_capacity(n);
  for i in buf.split_whitespace() {
    x.push(i.trim().parse::<T>().unwrap());
  }

  return x;
}

fn main() {
  println!("{}", read::<String>());
  println!("{:?}", read_into_vec::<f64>());
  println!("{:?}", read_into_vec_n::<i32>(2));
}

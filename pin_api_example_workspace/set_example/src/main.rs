use std::pin::Pin;

fn main() {
  let mut i: u8 = 8;
  let mut p = Pin::new(&mut i);
  p.set(9);
  assert_eq!(i, 9);
}

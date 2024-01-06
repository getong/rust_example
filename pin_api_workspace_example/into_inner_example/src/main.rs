use std::pin::Pin;

fn main() {
  let i: u8 = 8;
  let p = Pin::new(&i);
  assert_eq!(*Pin::into_inner(p), 8);
}

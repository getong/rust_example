use std::pin::Pin;

fn main() {
  let mut i: u8 = 8;
  let p = Pin::new(&mut i);
  *p.get_mut() = 9;
  assert_eq!(i, 9);
}

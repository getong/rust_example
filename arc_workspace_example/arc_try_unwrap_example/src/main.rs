use std::sync::Arc;
fn main() {
  // println!("Hello, world!");
  let x = Arc::new(3);
  assert_eq!(Arc::try_unwrap(x), Ok(3));

  let x = Arc::new(4);
  let y = Arc::clone(&x);
  // drop(x);
  assert_eq!(*Arc::try_unwrap(y).unwrap_err(), 4);
  drop(x);
  // println!("x: {:?}", x);
}

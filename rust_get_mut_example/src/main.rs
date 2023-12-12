fn main() {
  // println!("Hello, world!");
  let x = &mut [0, 1, 2];

  if let Some(elem) = x.get_mut(1) {
    *elem = 42;
  }
  assert_eq!(x, &[0, 42, 2]);
}

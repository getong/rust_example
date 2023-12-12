fn main() {
  // println!("Hello, world!");
  let (left, right) = bichannel::channel();

  // Send from the left to the right
  left.send(1).unwrap();
  assert_eq!(Ok(1), right.recv());

  // Send from the right to the left
  right.send(2).unwrap();
  assert_eq!(Ok(2), left.recv());
}

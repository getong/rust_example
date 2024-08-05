fn factorial(n: usize) -> usize {
  (1 .. n + 1).product()
}

fn main() {
  // println!("Hello, world!");
  let x = 10;
  let y = 10;
  let rx = &x;
  let ry = &y;
  let rrx = &rx;
  let rry = &ry;
  assert!(rrx <= rry);
  assert!(rrx == rry);

  assert!(rx == ry);
  assert!(!std::ptr::eq(rx, ry));
  assert!(rx == *rrx);

  let r = &factorial(6);
  assert_eq!(r + &1009, 1729);
}

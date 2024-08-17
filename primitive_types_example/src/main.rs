use primitive_types::U256;

fn main() {
  println!("Hello, world!");
  let a = U256::from(1);
  let b = U256::from(2);
  let c = a + b;
  println!("{} + {} = {}", a, b, c);
  let d = U256::MAX;
  println!("Max U256: {}", d);
  // will panic here
  // println!("{} + {} = {}", b, d, b + d);

  // Overflowing
  let (e, overflowed) = b.overflowing_add(d);
  if overflowed {
    println!("{} + {} overflowed", b, d);
  } else {
    println!("{} + {} = {}", b, d, e);
  }

  // Not overflowing
  let (f, overflowed) = a.overflowing_add(b);
  if overflowed {
    println!("{} + {} overflowed", a, b);
  } else {
    println!("{} + {} = {}", a, b, f);
  }
}

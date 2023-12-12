// copy from [Handling Integer Overflow in Rust](https://bmoxb.io/2023/01/28/integer-overflow-in-rust.html)
fn main() {
  // println!("Hello, world!");

  #[allow(arithmetic_overflow)]
  let x: u8 = 255 + 1;
  println!("{}", x);

  assert_eq!((250_u8).wrapping_add(10), 4);
  assert_eq!((120_i8).wrapping_add(10), -126);
  assert_eq!((300_u16).wrapping_mul(800), 43392);
  assert_eq!((-100_i8).wrapping_sub(100), 56);
  assert_eq!((8000_i32).wrapping_pow(5000), 640000);

  let (result, overflowed) = (250_u8).overflowing_add(10); // 4, true
  println!(
    "sum is {} where overflow {} occur",
    result,
    if overflowed { "did" } else { "did not" }
  );

  match (100_u8).checked_add(200) {
    Some(result) => println!("{result}"),
    None => panic!("overflowed!"),
  }

  assert_eq!(-32768_i32.saturating_sub(10), -32768);
  assert_eq!(200_u8.saturating_add(100), 255);

  use std::num::Wrapping;

  let x = Wrapping(125_u8);

  assert_eq!(x + Wrapping(200), Wrapping(69));
  assert_eq!(x - Wrapping(200), Wrapping(181));
  // x *= 5; // if we mutate the variable x then we can use primitive integer types - x is now 113

  // x / 5; // error! careful - we can only use primitives when we're assigning (i.e., using +=, -=, etc.)
}

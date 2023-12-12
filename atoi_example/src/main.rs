use atoi::atoi;
use atoi::FromRadix10;

fn main() {
  // println!("Hello, world!");
  assert_eq!(Some(42), atoi::<u32>(b"42"));

  // Parsing to digits from a slice
  assert_eq!((42, 2), u32::from_radix_10(b"42"));
}

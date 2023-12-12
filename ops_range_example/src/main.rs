use std::ops::Range;

fn main() {
  // println!("Hello, world!");
  assert_eq!((3..5), Range { start: 3, end: 5 });
  assert_eq!(3 + 4 + 5, (3..6).sum());

  let arr = [0, 1, 2, 3, 4];
  assert_eq!(arr[..], [0, 1, 2, 3, 4]);
  assert_eq!(arr[..3], [0, 1, 2]);
  assert_eq!(arr[..=3], [0, 1, 2, 3]);
  assert_eq!(arr[1..], [1, 2, 3, 4]);
  assert_eq!(arr[1..3], [1, 2]); // This is a `Range`
  assert_eq!(arr[1..=3], [1, 2, 3]);

  let r: Range<i32> = 1..5;
  println!("r: {:?}", r);
}

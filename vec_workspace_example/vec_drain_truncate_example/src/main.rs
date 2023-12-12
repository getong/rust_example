fn drain_example() {
  let mut vec = vec![1, 2, 3, 4, 5];
  let start = 1; // Start of the range (inclusive)
  let end = 3; // End of the range (exclusive)
  vec.drain(start..end);
  assert_eq!(vec![1, 4, 5], vec);
}

fn truncate_example() {
  let mut vec = vec![1, 2, 3, 4, 5];
  let keep_start = 1; // Start of the range to keep
  let keep_end = 3; // End of the range to keep

  vec.truncate(keep_end - keep_start);
  assert_eq!(vec![1, 2], vec);
}

fn main() {
  drain_example();
  truncate_example();
}

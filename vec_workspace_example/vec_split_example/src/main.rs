fn main() {
  // println!("Hello, world!");
  let slice = [10, 40, 33, 20];
  let mut iter = slice.split(|num| num % 3 == 0);

  assert_eq!(iter.next().unwrap().to_owned(), [10, 40]);
  assert_eq!(iter.next().unwrap().to_owned(), [20]);
  assert!(iter.next().is_none());
}

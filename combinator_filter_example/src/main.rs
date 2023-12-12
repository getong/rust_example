fn main() {
  // println!("Hello, world!");
  let s1 = Some(3);
  let s2 = Some(6);
  let n = None;

  let fn_is_even = |x: &i8| x % 2 == 0;

  assert_eq!(s1.filter(fn_is_even), n); // Some(3) -> 3 is not even -> None
  assert_eq!(s2.filter(fn_is_even), s2); // Some(6) -> 6 is even -> Some(6)
  assert_eq!(n.filter(fn_is_even), n); // None -> no value -> None
}

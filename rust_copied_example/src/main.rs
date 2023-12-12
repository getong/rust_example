fn main() {
  // println!("Hello, world!");
  let a = [1, 2, 3];

  let v_copied: Vec<_> = a.iter().copied().collect();

  // copied is the same as .map(|&x| x)
  let v_map: Vec<_> = a.iter().map(|&x| x).collect();

  assert_eq!(v_copied, vec![1, 2, 3]);
  assert_eq!(v_map, vec![1, 2, 3]);
}

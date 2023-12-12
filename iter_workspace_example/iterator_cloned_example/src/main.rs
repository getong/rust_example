fn main() {
  // println!("Hello, world!");
  let a = [1, 2, 3];

  let v_cloned: Vec<_> = a.iter().cloned().collect();

  // cloned is the same as .map(|&x| x), for integers
  let v_map: Vec<_> = a.iter().map(|&x| x).collect();

  assert_eq!(v_cloned, vec![1, 2, 3]);
  assert_eq!(v_map, vec![1, 2, 3]);
}

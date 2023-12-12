fn main() {
  // println!("Hello, world!");
  const V_DEFAULT: i8 = 1;

  let s = Some(10);
  let n: Option<i8> = None;
  let fn_closure = |v: i8| v + 2;

  assert_eq!(s.map_or(V_DEFAULT, fn_closure), 12);
  assert_eq!(n.map_or(V_DEFAULT, fn_closure), V_DEFAULT);
}

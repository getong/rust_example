fn f(a: Option<&mut i32>) {
  if let Some(&mut ref mut x) = a {
    *x = 6;
  }
  // ...
  if let Some(&mut ref mut x) = a {
    *x = 7;
  }
}

fn main() {
  // println!("Hello, world!");
  let mut i = 5;
  f(Some(&mut i));
  println!("i: {}", i);
}

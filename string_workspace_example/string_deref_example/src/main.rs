use std::ops::Deref;
fn main() {
  // println!("Hello, world!");
  let s: String = "hello".to_string();
  let s_ref: &str = (&s).deref();
  println!("s_ref: {:?}", s_ref);

  let s_ref2: &str = &*&s;
  println!("s_ref2: {:?}", s_ref2);
}

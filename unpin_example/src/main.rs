use std::mem;
use std::pin::Pin;

fn main() {
  // println!("Hello, world!");

  let mut string = "this".to_string();
  let mut pinned_string = Pin::new(&mut string);

  // We need a mutable reference to call `mem::replace`.
  // We can obtain such a reference by (implicitly) invoking `Pin::deref_mut`,
  // but that is only possible because `String` implements `Unpin`.
  let _ = mem::replace(&mut *pinned_string, "other".to_string());
  println!("pinned_string: {:?}", pinned_string);
}

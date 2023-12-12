use std::ops::{Deref, DerefMut};
struct Portal<T> {
  data: T,
}

impl<T> Deref for Portal<T> {
  type Target = T;
  fn deref(&self) -> &T {
    &self.data
  }
}

impl<T> DerefMut for Portal<T> {
  fn deref_mut(&mut self) -> &mut T {
    &mut self.data
  }
}

fn main() {
  // println!("Hello, world!");
  let mut p = Portal::<i32> { data: 42 };
  *p = 0;
  println!("{}", 100 + *p);
}

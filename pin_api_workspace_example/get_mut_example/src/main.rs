use std::{marker::PhantomPinned, pin::Pin};

#[derive(Debug)]
pub struct Test {
  a: String,
  _marker: PhantomPinned,
}

fn main() {
  let mut i: u8 = 8;
  let p = Pin::new(&mut i);
  *p.get_mut() = 9;
  assert_eq!(i, 9);

  let mut t = Test {
    a: String::from("abc"),
    _marker: PhantomPinned,
  };

  let mut p2 = unsafe { Pin::new_unchecked(&mut t) };

  unsafe {
    let inner = p2.as_mut().get_unchecked_mut();
    inner.a = String::from("def");
  }

  println!("p2: {:?}", p2);
}

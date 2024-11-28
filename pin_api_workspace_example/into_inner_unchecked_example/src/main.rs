use std::{marker::PhantomPinned, pin::Pin};

#[derive(PartialEq, Debug)]
struct Foo {
  x: i32,
  _pin: PhantomPinned,
}

fn main() {
  let twos = Foo {
    x: 2,
    _pin: PhantomPinned,
  };

  let ptr = unsafe { Pin::new_unchecked(&twos) };
  unsafe {
    assert_eq!(Pin::into_inner_unchecked(ptr), &twos);
  }
}

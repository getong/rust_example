use std::marker::PhantomPinned;
use std::pin::Pin;

#[derive(PartialEq, Debug)]
struct Foo {
  x: i32,
  _pin: PhantomPinned,
}

fn main() {
  let mut i = 9;
  let mut pin = Pin::new(&mut i);
  *pin.as_mut() = 10;
  assert_eq!(i, 10);

  // !Unpin
  let mut twos = Foo {
    x: 2,
    _pin: PhantomPinned,
  };
  // let mut pin = unsafe { Pin::new_unchecked(&mut twos) };
  let pin = unsafe { Pin::new_unchecked(&mut twos) };
  unsafe {
    // *pin.as_mut().get_unchecked_mut() = Foo {
    *pin.get_unchecked_mut() = Foo {
      x: 3,
      _pin: PhantomPinned,
    }
  };
  assert_eq!(twos.x, 3);
}

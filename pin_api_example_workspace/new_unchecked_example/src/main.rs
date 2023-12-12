use std::marker::PhantomPinned;
use std::pin::Pin;

struct Foo {
  x: i32,
  _pin: PhantomPinned,
}

fn main() {
  let mut twos = Foo {
    x: 2,
    _pin: PhantomPinned,
  };
  let mut ptr = unsafe { Pin::new_unchecked(&mut twos) };
  let mut_ref: Pin<&mut Foo> = ptr.as_mut();
  unsafe {
    mut_ref.get_unchecked_mut().x = 22;
  }
  println!("twos: {}", twos.x);
  twos.x = 222;
  println!("twos: {}", twos.x);
}

use std::{marker::PhantomPinned, pin::Pin, ptr::NonNull};

#[derive(PartialEq, Debug)]
struct Foo {
  x: i32,
  _pin: PhantomPinned,
}

#[derive(Debug)]
struct Unmovable {
  data: String,
  slice: NonNull<String>,
  _pin: PhantomPinned,
}

impl Unmovable {
  fn new(data: String) -> Pin<Box<Self>> {
    let res = Unmovable {
      data,
      slice: NonNull::dangling(),
      _pin: PhantomPinned,
    };
    let mut boxed = Box::pin(res);

    let slice = NonNull::from(&boxed.data);

    let mut_ref: Pin<&mut Self> = boxed.as_mut();
    unsafe {
      mut_ref.get_unchecked_mut().slice = slice;
    }
    boxed
  }
}

fn pointer_pin_example() {
  // !Unpin , pointer
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

fn not_pointer_pin_example() {
  let mut still_unmoved = Unmovable::new("hello".to_string());

  let mut new_unmoved = Unmovable::new("world".to_string());
  println!(
    "still_unmoved: {:?}, new_unmoved:{:?}",
    still_unmoved, new_unmoved
  );

  std::mem::swap(&mut still_unmoved, &mut new_unmoved);
  println!(
    "after swap, still_unmoved: {:?}, new_unmoved:{:?}",
    still_unmoved, new_unmoved
  );
}

fn main() {
  pointer_pin_example();

  not_pointer_pin_example();
}

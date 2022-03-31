use std::marker::PhantomPinned;
use std::pin::Pin;

#[derive(PartialEq, Debug)]
struct Foo {
    x: i32,
    _pin: PhantomPinned,
}

impl Foo {
    fn pin_get_field(self: Pin<&mut Self>) -> Pin<&mut i32> {
        // This is okay because `field` is pinned when `self` is.
        unsafe { self.map_unchecked_mut(|s| &mut s.x) }
    }
}

fn main() {
    // println!("Hello, world!");
    let mut twos = Foo {
        x: 2,
        _pin: PhantomPinned,
    };

    let ptr = unsafe { Pin::new_unchecked(&mut twos) };

    let mut x: Pin<&mut i32> = ptr.pin_get_field();
    *x = 3;
    assert_eq!(*x, 3);

    println!("twos: {:?}", twos);
}

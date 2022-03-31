use std::pin::Pin;

#[derive(PartialEq, Debug)]
struct Foo {
    x: i32,
}

impl Foo {
    fn pin_get_field(self: Pin<&Self>) -> Pin<&i32> {
        unsafe { self.map_unchecked(|s| &s.x) }
    }
}

fn main() {
    // println!("Hello, world!");
    let twos = Foo { x: 2 };

    let ptr = Pin::new(&twos);

    let x: Pin<&i32> = ptr.pin_get_field();
    assert_eq!(*x, 2);

    println!("twos: {:?}", twos);
}

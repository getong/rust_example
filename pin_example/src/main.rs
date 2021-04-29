use std::marker::PhantomPinned;

use std::pin::Pin;
use std::ptr::NonNull;

#[derive(Debug)]
struct Unmovable {
    data: String,
    slice: NonNull<String>,
    _pin: PhantomPinned,
}

impl Unpin for Unmovable {}

impl Unmovable {
    fn new(data: String) -> Pin<Box<Self>> {
        let res = Unmovable {
            data,
            slice: NonNull::dangling(),
            _pin: PhantomPinned,
        };
        let mut boxed = Box::pin(res);

        let slice = NonNull::from(&boxed.data);

        unsafe {
            let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut boxed);
            Pin::get_unchecked_mut(mut_ref).slice = slice;
        }
        boxed
    }
}

fn main() {
    let unmoved = Unmovable::new("hello".to_string());

    let mut still_unmoved = unmoved;

    let mut new_unmoved = Unmovable::new("world".to_string());
    println!(
        "still_unmoved: {:?}, new_unmoved:{:?}",
        still_unmoved, new_unmoved
    );

    std::mem::swap(&mut *still_unmoved, &mut *new_unmoved);
    println!(
        "after swap, still_unmoved: {:?}, new_unmoved:{:?}",
        still_unmoved, new_unmoved
    );
}

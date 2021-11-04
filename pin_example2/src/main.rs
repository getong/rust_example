use std::marker::PhantomPinned;
use std::pin::Pin;

#[derive(Debug)]
pub struct Test {
    a: String,
    b: *const String,
    _marker: PhantomPinned,
}

impl Test {
    fn new(txt: &str) -> Pin<Box<Self>> {
        let t = Test {
            a: String::from(txt),
            b: std::ptr::null(),
            _marker: PhantomPinned,
        };
        let mut boxed = Box::pin(t);
        let self_ptr: *const String = &boxed.as_ref().a;
        unsafe { boxed.as_mut().get_unchecked_mut().b = self_ptr };

        boxed
    }

    pub fn a<'a>(self: Pin<&'a Self>) -> &'a str {
        &self.get_ref().a
    }

    pub fn b<'a>(self: Pin<&'a Self>) -> &'a String {
        unsafe { &*(self.b) }
    }
}

pub fn main() {
    let mut test1 = Test::new("test1");
    let mut test2 = Test::new("test2");

    println!(
        "before swap a: {}, b: {}",
        test1.as_ref().b(),
        test2.as_ref().b()
    );
    // std::mem::swap(test1.get_mut(), test2.get_mut());
    // std::mem::swap(&mut *test1, &mut *test2);
    // std::mem::swap(&mut test1.as_mut(), &mut test2.as_mut());
    std::mem::swap(&mut test1, &mut test2);
    println!(
        "after swap a: {}, b: {}",
        test1.as_ref().b(),
        test2.as_ref().b()
    );
}

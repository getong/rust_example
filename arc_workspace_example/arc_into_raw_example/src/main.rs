use std::sync::Arc;

fn main() {
    let x = Arc::new("hello".to_owned());
    let x_ptr = Arc::into_raw(x);
    unsafe { assert_eq!(*x_ptr, "hello".to_owned()) };
}

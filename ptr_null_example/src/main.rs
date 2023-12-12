use std::ptr;
fn main() {
  // println!("Hello, world!");

  let p: *mut i32 = ptr::null_mut();
  assert!(p.is_null());
}

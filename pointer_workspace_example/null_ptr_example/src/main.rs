use std::ptr;

fn main() {
  let p: *const i32 = ptr::null();
  assert!(p.is_null());
  let p: *mut i32 = ptr::null_mut();
  assert!(p.is_null());
}

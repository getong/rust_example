use std::ffi::CStr;
use std::os::raw::c_char;

extern "C" {
  static environ: *mut *mut c_char;
}

fn main() {
  // println!("Hello, world!");
  unsafe {
    if !environ.is_null() && !(*environ).is_null() {
      let var = CStr::from_ptr(*environ);
      println!("first environment variable: {}", var.to_string_lossy())
    }
  }
}

// Use all the autocxx types which might be handy.

use autocxx::prelude::*;
// autocxx 0.30 generates unsafe functions without explicit unsafe blocks.
// Keep this compatibility allowance scoped to generated bindings only.
#[allow(unsafe_op_in_unsafe_fn)]
mod bindings {
  use autocxx::prelude::*;

  include_cpp! {
    #include "input.h"
    safety!(unsafe_ffi)
        generate!("DoMath")
        generate!("Goat")
  }

  pub(super) use ffi::{DoMath, Goat};
}

use bindings as ffi;

fn main() {
  println!("Hello, world! - C++ math should say 12={}", ffi::DoMath(4));
  let mut goat = ffi::Goat::new().within_box();
  goat.as_mut().add_a_horn();
  goat.as_mut().add_a_horn();
  assert_eq!(
    goat.describe().as_ref().unwrap().to_string_lossy(),
    "This goat has 2 horns."
  );
}

// Use all the autocxx types which might be handy.


use autocxx::prelude::*;
include_cpp! {
    #include "input.h"
    safety!(unsafe_ffi)
        generate!("DoMath")
        generate!("Goat")
}

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

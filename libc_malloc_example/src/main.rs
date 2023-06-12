use libc::{malloc, size_t};
use std::mem;
use std::ptr;

fn main() {
    // How ugly it is to pretend Rust is unsafe C.
    unsafe {
        let mut orig: *mut i32 = malloc(mem::size_of::<i32>() as size_t) as *mut i32;
        ptr::write(&mut *orig, 5i32);

        println!("{}", *orig);

        orig = ptr::null::<i32>() as *mut i32;

        // null pointer crash!
        println!("{}", *orig);
    }
}

// copy from https://conscientiousprogrammer.com/blog/2014/12/21/how-to-think-about-rust-ownership-versus-c-plus-plus-unique-ptr/
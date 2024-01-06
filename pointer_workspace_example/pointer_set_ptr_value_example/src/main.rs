#![feature(set_ptr_value)]
use std::fmt::Debug;

fn main() {
  // println!("Hello, world!");

  let arr: [i32; 3] = [1, 2, 3];
  let mut ptr = arr.as_ptr() as *const dyn Debug;
  let thin = ptr as *const u8;
  unsafe {
    println!("before {:?}", &*ptr); // will print "1"
    ptr = ptr.set_ptr_value(thin.add(4));
    println!("mid {:?}", &*ptr); // will print "2"
    ptr = ptr.set_ptr_value(thin.add(8));
    println!("after {:?}", &*ptr); // will print "3"
  }

  println!("arr: {:?}", arr);
}

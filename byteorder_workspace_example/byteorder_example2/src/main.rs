use std::{mem, slice};

use byteorder::{LittleEndian, WriteBytesExt};

fn main() {
  let i: i64 = 12345;
  let mut bs = [0u8; mem::size_of::<i64>()];
  bs.as_mut()
    .write_i64::<LittleEndian>(i)
    .expect("Unable to write");

  for i in &bs {
    println!("{}", i);
  }

  println!("newline -----------");
  from_raw_parts();
}

fn from_raw_parts() {
  let i: i64 = 12345;
  let ip: *const i64 = &i;
  let bp: *const u8 = ip as *const _;
  let bs: &[u8] = unsafe { slice::from_raw_parts(bp, mem::size_of::<i64>()) };

  for i in bs {
    println!("{}", i);
  }
}

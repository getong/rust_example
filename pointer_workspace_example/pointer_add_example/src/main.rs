fn main() {
  // println!("Hello, world!");
  let s: &str = "123";
  let ptr: *const u8 = s.as_ptr();

  unsafe {
    println!("{}", *ptr.add(1) as char);
    println!("{}", *ptr.add(2) as char);
  }
}

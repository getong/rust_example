fn main() {
  // println!("Hello, world!");
  let mut s1 = String::from("hello");
  println!("String pointing to heap location {:?}", s1.as_ptr());
  println!("String stored on stack {:?}", std::ptr::addr_of!(s1));
  //    {
  //      s1; // release it
  //}
  let s2 = String::from("hello, world");
  // a long string literal will allocate a different area on heap.
  // let s2 = String::from("123456789012345678901234567890");
  println!("String pointing to heap location {:?}", s2.as_ptr());
  println!("String stored on stack {:?}", std::ptr::addr_of!(s2));

  s1 = s2;
  // a shadow variable will allocate a different area on stack.
  // let s1 = s2;
  println!("String pointing to heap location {:?}", s1.as_ptr());
  println!("String stored on stack {:?}", std::ptr::addr_of!(s1));
}

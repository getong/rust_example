fn main() {
  // println!("Hello, world!");
  let mut a = [0, 1];
  let from = a.as_ptr();
  unsafe {
    let to = a.as_mut_ptr().add(1); // `from` gets invalidated here
    std::ptr::copy_nonoverlapping(from, to, 1);
  }
  println!("a : {:?}", a);

  let mut a = [0, 1];
  unsafe {
    let to = a.as_mut_ptr().add(1);
    to.write(0);
    let from = a.as_ptr();
    std::ptr::copy_nonoverlapping(from, to, 1);
  }
  println!("a : {:?}", a);
}

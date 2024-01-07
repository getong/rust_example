fn zeroed_vector<T>(len: usize) -> Vec<T> {
  let mut vec = Vec::with_capacity(len);

  unsafe {
    std::ptr::write_bytes(vec.as_mut_ptr(), 0, len);
    vec.set_len(len);
  }
  vec
}
fn main() {
  //println!("Hello, world!");
  let v: Vec<usize> = zeroed_vector(100_000);
  assert!(v.iter().all(|&u| u == 0));
}

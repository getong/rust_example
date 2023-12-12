fn main() {
  // println!("Hello, world!");
  let (s, r) = async_channel::unbounded();

  s.try_send(7).unwrap();
  assert_eq!(r.try_recv(), Ok(7));
}

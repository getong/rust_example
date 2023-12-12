fn main() {
  // println!("Hello, world!");
  let mut arr = [0u8; 10];
  arr[0] = 1;
  // let mut sl = &[0u8;10][..];
  let sl = &mut [0u8; 10][..];
  sl[0] = 1;
}

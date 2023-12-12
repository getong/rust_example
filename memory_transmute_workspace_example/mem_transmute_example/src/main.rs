fn main() {
  // println!("Hello, world!");

  let raw_bytes: [u8; 4] = [0x78, 0x56, 0x34, 0x12];
  let num: u32 = unsafe { std::mem::transmute::<[u8; 4], u32>(raw_bytes) };
  let ret = u32::from_ne_bytes(raw_bytes);

  assert_eq!(num, 0x12345678);
  assert_eq!(num, ret);
  // num = 305419896
  //println!("num = {}", num);
  assert_eq!(305419896, num);

  another_example();
}

fn another_example() {
  let a: f32 = 42.42;
  let frankentype: u32 = unsafe { std::mem::transmute(a) };

  println!("frankentype is {}", frankentype);
  println!("{:032b}", frankentype);

  let b: f32 = unsafe { std::mem::transmute(frankentype) };

  println!("b is {}", b);
  assert_eq!(a, b);
}

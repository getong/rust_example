fn main() {
  // println!("Hello, world!");

  let a = 123_i32;
  println!("123 in bytes : {:?}", bytemuck::bytes_of(&a));

  let mut b = 123_i32;
  println!("123 in bytes : {:?}", bytemuck::bytes_of_mut(&mut b));
}

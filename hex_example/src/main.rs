fn main() {
  // println!("Hello, world!");
  let hex_str: &str = "hello world";
  let hex = hex::encode(hex_str);

  println!("hex: {:?}", hex);
}

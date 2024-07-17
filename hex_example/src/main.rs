fn main() {
  let hex_str: &str = "hello world";
  let hex = hex::encode(hex_str);

  println!("hex: {:?}", hex);

  let bytes = hex::decode(hex).expect("Decoding failed");
  // Convert bytes back to a string
  let original_str = String::from_utf8(bytes).expect("Invalid UTF-8");

  println!("Original string: {:?}", original_str);
  assert_eq!(hex_str, original_str);
}

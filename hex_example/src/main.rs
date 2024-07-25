use hex::ToHex;

fn main() {
  let hex_str: &str = "hello world";
  let hex = hex::encode(hex_str);

  println!("hex: {:?}", hex);

  // Add the "0x" prefix
  let prefixed_hex_string = format!("0x{}", hex);
  println!("prefixed_hex_string: {:?}", prefixed_hex_string);

  let bytes = hex::decode(hex).expect("Decoding failed");
  // Convert bytes back to a string
  let original_str = String::from_utf8(bytes).expect("Invalid UTF-8");

  println!("Original string: {:?}", original_str);
  assert_eq!(hex_str, original_str);

  println!("{}", "Hello world!".encode_hex::<String>());

  if let Ok(bytes) = hex::decode(prefixed_hex_string) {
    // Convert bytes back to a string
    if let Ok(original_str) = String::from_utf8(bytes) {
      println!("Original string: {:?}", original_str);
      assert_eq!(hex_str, original_str);
    } else {
      println!("can not run from_utf8 operation");
    }
  } else {
    println!("can not decode from prefixed_hex_string");
  }
}

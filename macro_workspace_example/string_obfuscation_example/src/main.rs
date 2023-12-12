use std::time::Duration;
use string_obfuscation_example::xor_string;

fn main() {
  println!("Content: {}", xor_string!("Top_Secret"));
  // Now "Top_Secret" is deobfuscated and in memory
  // But "Super_Top_Secret" is still obfuscated
  std::thread::sleep(Duration::from_millis(10_000));
  println!("Content: {}", xor_string!("Super_Top_Secret"));
}

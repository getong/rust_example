use ethers::types::U256;

fn format_u256_with_underscores(num: U256, width: usize) -> String {
  // Convert to a decimal string
  let mut num_str = num.to_string();

  // Add leading zeros if necessary
  while num_str.len() < width {
    num_str.insert(0, '0');
  }

  // Insert underscores every three digits
  let mut formatted = String::new();
  for (i, ch) in num_str.chars().rev().enumerate() {
    if i != 0 && i % 3 == 0 {
      formatted.insert(0, '_');
    }
    formatted.insert(0, ch);
  }

  formatted
}

fn main() {
  let number = U256::from_dec_str("123456789").unwrap();
  let formatted_number = format_u256_with_underscores(number, 15); // Adjust width as needed
  println!("{}", formatted_number); // Outputs: 000_000_123_456_789

  let formatted_number = format_u256_with_underscores(number, 9); // Adjust width as needed
  println!("{}", formatted_number); // Outputs: 123_456_789
}

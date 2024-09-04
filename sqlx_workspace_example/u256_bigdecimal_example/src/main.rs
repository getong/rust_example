use bigdecimal::BigDecimal;
use ethers::types::U256;
use std::str::FromStr;

fn u256_to_bigdecimal(value: U256) -> BigDecimal {
  // Convert U256 to string
  let value_str = value.to_string();

  // Parse the string into BigDecimal
  BigDecimal::from_str(&value_str).unwrap()
}

#[tokio::main]
async fn main() {
  // Example usage
  let u256_value = U256::from(123456789012345678901234567890u128); // Example U256 value

  let big_decimal_value = u256_to_bigdecimal(u256_value);

  println!("BigDecimal value: {}", big_decimal_value);
}

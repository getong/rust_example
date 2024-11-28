use std::str::FromStr;

use bigdecimal::BigDecimal;
use ethers::types::U256;

fn u256_to_bigdecimal(value: U256) -> BigDecimal {
  // Convert U256 to string
  let value_str = value.to_string();

  // Parse the string into BigDecimal
  BigDecimal::from_str(&value_str).unwrap()
}

fn bigdecimal_to_u256(value: BigDecimal) -> U256 {
  // Convert BigDecimal to string
  let value_str = value.to_string();

  // Parse the string into U256
  U256::from_dec_str(&value_str).unwrap()
}

#[tokio::main]
async fn main() {
  // Example usage
  let u256_value: U256 = U256::from_dec_str("123456789012345678901234567890").unwrap(); // Example U256 value

  // Convert U256 to BigDecimal
  let big_decimal_value: BigDecimal = u256_to_bigdecimal(u256_value);
  println!("BigDecimal value: {}", big_decimal_value);

  // Convert BigDecimal back to U256
  let u256_converted_back: U256 = bigdecimal_to_u256(big_decimal_value.clone());
  println!("U256 value: {}", u256_converted_back);
  let big_decimal_value_back: BigDecimal = u256_to_bigdecimal(u256_converted_back.clone());

  assert_eq!(u256_value, u256_converted_back);
  assert_eq!(big_decimal_value, big_decimal_value_back);
}

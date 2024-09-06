use bigdecimal::BigDecimal;
use std::str::FromStr;

fn main() {
  let input1 = "0.8";
  let input2 = "1.2";

  // Convert the input strings to BigDecimal
  let dec1 = BigDecimal::from_str(&input1).unwrap();
  let dec2 = BigDecimal::from_str(&input2).unwrap();

  // Perform arithmetic operations
  let sum = &dec1 + &dec2; // Addition
  let difference = &dec1 - &dec2; // Subtraction
  let product = &dec1 * &dec2; // Multiplication
  let quotient = &dec1 / &dec2; // Division

  // Print the results
  println!("{} + {} = {}", dec1, dec2, sum);
  println!("{} - {} = {}", dec1, dec2, difference);
  println!("{} * {} = {}", dec1, dec2, product);
  println!("{} / {} = {}", dec1, dec2, quotient);
  // Initialize BigDecimal with zero
  let zero = BigDecimal::from(0);

  // Alternatively, you can use from_str
  let zero_from_str = BigDecimal::from_str("0").unwrap();

  println!("BigDecimal zero: {}", zero);
  println!("BigDecimal zero from str: {}", zero_from_str);
}

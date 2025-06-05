use std::str::FromStr;

use bigdecimal::{num_traits::Pow, BigDecimal, RoundingMode};

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

  let input = "12000000000000000000000000000";
  // Convert to BigDecimal and convert back to string
  let big_dec = BigDecimal::from_str(input).unwrap();
  let big_dec_str = big_dec.to_plain_string();

  println!("BigDecimal from input: {}", big_dec);
  println!("BigDecimal back to string: {}", big_dec_str);

  // Raise BigDecimal to the power of 10^6
  // let base = BigDecimal::from_str("2").unwrap();
  // let exponent = 1_000_000;
  // let result = base.pow(exponent);

  // println!("{} raised to the power of {} is {}", base, exponent, result);
  assert_eq!(Pow::pow(10u32, 2u32), 100);
  println!("d is {:?}", Pow::pow(10u32, 2u32));
  let a = BigDecimal::from(1_000_000u64);
  let b = BigDecimal::from(1_000_000_000_000_000_000u64);
  let c = a / b;
  println!("c is {:?}", c);

  // Round function examples
  let decimal_to_round = BigDecimal::from_str("3.14159265").unwrap();

  // Round to 2 decimal places
  let rounded_2 = decimal_to_round.round(2);
  println!("Rounded to 2 decimal places: {}", rounded_2);

  // Round to 4 decimal places
  let rounded_4 = decimal_to_round.round(4);
  println!("Rounded to 4 decimal places: {}", rounded_4);

  // Round with different rounding modes
  let test_value = BigDecimal::from_str("2.5").unwrap();
  let rounded_half_up = test_value.with_scale_round(0, RoundingMode::HalfUp);
  let rounded_half_even = test_value.with_scale_round(0, RoundingMode::HalfEven);
  let rounded_half = test_value.round(0);

  println!("2.5 rounded half up: {}", rounded_half_up);
  println!("2.5 rounded half even: {}", rounded_half_even);
  println!("2.5 rounded : {}", rounded_half);
}

use num_bigint::{BigInt, BigUint, Sign};

fn main() {
  println!("num-bigint: arbitrary precision integer examples");
  println!("================================================\n");

  explain_why_bigints_matter();
  show_biguint_arithmetic();
  show_factorial();
  show_power_and_modpow();
  show_signed_bigint();
  show_radix_and_bytes();
}

fn explain_why_bigints_matter() {
  println!("1. Why use num-bigint?");
  println!(
    "   Rust primitive integers such as u128 have fixed limits. BigUint and BigInt grow as \
     needed, so they are useful for cryptography, combinatorics, scientific calculations, and any \
     domain where values can exceed fixed-width integers.\n"
  );

  println!("   u128::MAX = {}", u128::MAX);
  println!("   2^200     = {}\n", BigUint::from(2u32).pow(200));
}

fn show_biguint_arithmetic() {
  println!("2. BigUint for non-negative integers");

  let a =
    BigUint::parse_bytes(b"123456789012345678901234567890", 10).expect("valid decimal BigUint");
  let b =
    BigUint::parse_bytes(b"987654321098765432109876543210", 10).expect("valid decimal BigUint");

  println!("   a     = {a}");
  println!("   b     = {b}");
  println!("   a + b = {}", &a + &b);
  println!("   b - a = {}", &b - &a);
  println!("   a * b = {}", &a * &b);
  println!("   b / a = {}", &b / &a);
  println!("   b % a = {}\n", &b % &a);
}

fn show_factorial() {
  println!("3. Factorial without overflow");

  let value = factorial(100);
  println!(
    "   100! has {} decimal digits",
    value.to_str_radix(10).len()
  );
  println!("   100! = {value}\n");
}

fn factorial(n: u32) -> BigUint {
  (1 ..= n)
    .map(BigUint::from)
    .fold(BigUint::from(1u32), |acc, value| acc * value)
}

fn show_power_and_modpow() {
  println!("4. Power and modular exponentiation");

  let base = BigUint::from(7u32);
  let exponent = BigUint::from(560u32);
  let modulus = BigUint::from(561u32);

  println!("   7^20          = {}", base.pow(20));
  println!("   7^560 mod 561 = {}\n", base.modpow(&exponent, &modulus));
}

fn show_signed_bigint() {
  println!("5. BigInt for signed integers");

  let debt = BigInt::from(-123_456_789i64);
  let credit = BigInt::from(987_654_321i64);
  let balance = &credit + &debt;
  let constructed = BigInt::from_biguint(Sign::Minus, BigUint::from(42u32));

  println!("   debt                  = {debt}");
  println!("   credit                = {credit}");
  println!("   credit + debt         = {balance}");
  println!("   explicit negative 42  = {constructed}\n");
}

fn show_radix_and_bytes() {
  println!("6. Parsing, formatting, and byte conversion");

  let from_hex = BigUint::parse_bytes(b"deadbeefcafebabe", 16).expect("valid hex BigUint");
  let bytes = from_hex.to_bytes_be();
  let restored = BigUint::from_bytes_be(&bytes);

  println!("   parsed hex      = {}", from_hex.to_str_radix(16));
  println!("   decimal         = {from_hex}");
  println!("   big-endian bytes = {:02x?}", bytes);
  println!("   restored equal  = {}", restored == from_hex);
}

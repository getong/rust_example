fn main() {
  let input = Some("42");

  if let Some(s) = input
    && let Ok(n) = s.parse::<i32>()
    && n > 10
    && n < 100
  {
    println!("Parsed number is between 10 and 100: {}", n);
  } else {
    println!("Input is not a valid number in the range 10..100");
  }
}

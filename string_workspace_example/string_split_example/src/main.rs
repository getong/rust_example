fn main() {
  let input = "abc |||cde";
  let first_part = input.split("|||").next().unwrap_or(input).trim();
  println!("first part of {} || is {}", input, first_part);

  let input = "abc cde";
  let first_part = input.split("|||").next().unwrap().trim();
  println!("first part of {} || is {}", input, first_part);
}

fn main() {
  let input = "42";
  let result = input.parse::<i32>();

  if let Ok(deserialized) = &result {
    println!(
      "First check: Parsing successful, value is: {}",
      deserialized
    );
  } else {
    println!("First check: Parsing failed");
  }

  // Corrected second if let block
  if let Ok(deserialized) = result.as_ref() {
    println!(
      "Second check: Parsing successful, value is: {}",
      deserialized
    );
  } else {
    println!("Second check: Parsing failed");
  }

  let input = "42";
  let mut result = input.parse::<i32>();

  if let Ok(deserialized) = &mut result {
    println!(
      "First check: Parsing successful, value is: {}",
      deserialized
    );
    *deserialized += 1;
  } else {
    println!("First check: Parsing failed");
  }

  // Corrected second if let block
  if let Ok(deserialized) = result.as_mut() {
    println!(
      "Second check: Parsing successful, value is: {}",
      deserialized
    );
    *deserialized += 1;
  } else {
    println!("Second check: Parsing failed");
  }
}

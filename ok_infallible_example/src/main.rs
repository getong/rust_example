use std::convert::Infallible;

fn always_successful() -> Result<i32, Infallible> {
  // Some computation or logic that cannot fail
  let result = 42;

  // Explicitly state that the error type is Infallible
  Ok::<i32, Infallible>(result)
}

fn main() {
  match always_successful() {
    Ok(value) => println!("Success with value: {}", value),
    Err(_) => println!("This will never happen"),
  }
}

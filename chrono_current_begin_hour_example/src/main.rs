use chrono::{Timelike, Utc};

fn main() {
  // Get the current time
  let now = Utc::now().naive_utc();

  // Get the beginning of the current hour
  let beginning_of_hour = Utc::now()
    .naive_utc()
    .with_minute(0)
    .unwrap()
    .with_second(0)
    .unwrap();

  // Print the result
  println!("Current time: {}", now);
  println!("Beginning of the hour: {}", beginning_of_hour);
}

use chrono::{Duration, Timelike, Utc};

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

  let last_hour = beginning_of_hour - Duration::hours(1);

  // Print the result
  println!("Current time: {}", now);
  println!(
    "Beginning of the hour: {}, timestamp is {}",
    beginning_of_hour,
    beginning_of_hour.and_utc().timestamp()
  );

  println!(
    "last hour: {}, timestamp is {}",
    last_hour,
    last_hour.and_utc().timestamp()
  );
}

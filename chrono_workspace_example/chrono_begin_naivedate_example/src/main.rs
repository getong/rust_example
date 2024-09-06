use chrono::{Datelike, NaiveDate, NaiveDateTime, Utc};

fn main() {
  // Get the current date in UTC
  let now = Utc::now().naive_utc();

  // Get the beginning of today
  let beginning_of_today: NaiveDateTime =
    NaiveDate::from_ymd_opt(now.year(), now.month(), now.day())
      .unwrap_or_default()
      .and_hms_opt(0, 0, 0)
      .unwrap_or_default();

  // Print the result
  println!("Current time: {}", now);
  println!(
    "Beginning of today: {}, timestamp is {}",
    beginning_of_today,
    beginning_of_today.and_utc().timestamp()
  );
}

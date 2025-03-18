use chrono::{Datelike, NaiveDate, TimeZone, Utc};

fn beginning_of_sunday() -> i64 {
  let now = Utc::now();
  let weekday = now.weekday().num_days_from_sunday();

  let sunday = now.date_naive() - chrono::Duration::days(weekday.into());
  let sunday_midnight = NaiveDate::from_ymd_opt(sunday.year(), sunday.month(), sunday.day())
    .unwrap()
    .and_hms_opt(0, 0, 0)
    .unwrap();

  Utc.from_utc_datetime(&sunday_midnight).timestamp()
}

fn main() {
  let timestamp = beginning_of_sunday();
  println!("Beginning timestamp of this Sunday: {}", timestamp);
}

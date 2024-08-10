use chrono::{offset::FixedOffset, Local, LocalResult, NaiveDate, TimeZone, Utc, Weekday};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
  println!("Hello, time now is {:?}", Utc::now());

  let dt = Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap(); // `2014-07-08T09:10:11Z`
  assert_eq!(
    dt,
    NaiveDate::from_ymd_opt(2014, 7, 8)
      .ok_or("Invalid date")?
      .and_hms_opt(9, 10, 11)
      .ok_or("Invalid time")?
      .and_utc()
  );

  // July 8 is 188th day of the year 2014 (`o` for "ordinal")
  assert_eq!(
    dt,
    NaiveDate::from_yo_opt(2014, 189)
      .ok_or("Invalid ordinal date")?
      .and_hms_opt(9, 10, 11)
      .ok_or("Invalid time")?
      .and_utc()
  );

  // July 8 is Tuesday in ISO week 28 of the year 2014.
  assert_eq!(
    dt,
    NaiveDate::from_isoywd_opt(2014, 28, Weekday::Tue)
      .ok_or("Invalid ISO week date")?
      .and_hms_opt(9, 10, 11)
      .ok_or("Invalid time")?
      .and_utc()
  );

  let dt = NaiveDate::from_ymd_opt(2014, 7, 8)
    .ok_or("Invalid date")?
    .and_hms_milli_opt(9, 10, 11, 12)
    .ok_or("Invalid time")?
    .and_utc(); // `2014-07-08T09:10:11.012Z`

  assert_eq!(
    dt,
    NaiveDate::from_ymd_opt(2014, 7, 8)
      .ok_or("Invalid date")?
      .and_hms_micro_opt(9, 10, 11, 12_000)
      .ok_or("Invalid time")?
      .and_utc()
  );

  assert_eq!(
    dt,
    NaiveDate::from_ymd_opt(2014, 7, 8)
      .ok_or("Invalid date")?
      .and_hms_nano_opt(9, 10, 11, 12_000_000)
      .ok_or("Invalid time")?
      .and_utc()
  );

  // Handle LocalResult for `local_dt`
  let local_dt = match Local.from_local_datetime(
    &NaiveDate::from_ymd_opt(2014, 7, 8)
      .ok_or("Invalid date")?
      .and_hms_milli_opt(9, 10, 11, 12)
      .ok_or("Invalid time")?,
  ) {
    LocalResult::Single(dt) => dt,
    LocalResult::Ambiguous(_, _) => return Err("Ambiguous local datetime".into()),
    LocalResult::None => return Err("Invalid local datetime".into()),
  };

  println!("Local datetime is {:?}", local_dt);

  // Handle LocalResult for `fixed_dt`
  let fixed_dt = match FixedOffset::east_opt(9 * 3600)
    .ok_or("Invalid offset")?
    .from_local_datetime(
      &NaiveDate::from_ymd_opt(2014, 7, 8)
        .ok_or("Invalid date")?
        .and_hms_milli_opt(18, 10, 11, 12)
        .ok_or("Invalid time")?,
    ) {
    LocalResult::Single(dt) => dt,
    LocalResult::Ambiguous(_, _) => return Err("Ambiguous fixed offset datetime".into()),
    LocalResult::None => return Err("Invalid fixed offset datetime".into()),
  };

  assert_eq!(dt, fixed_dt);

  Ok(())
}

use chrono::{
  offset::FixedOffset, DateTime, Local, LocalResult, NaiveDate, TimeZone, Utc, Weekday,
};
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
  datetime_function_example();
  Ok(())
}

fn datetime_function_example() {
  let dt = Utc.with_ymd_and_hms(2014, 11, 28, 12, 0, 9).unwrap();
  let fixed_dt = dt.with_timezone(&FixedOffset::east_opt(9 * 3600).unwrap());

  // method 1
  assert_eq!(
    "2014-11-28T12:00:09Z".parse::<DateTime<Utc>>(),
    Ok(dt.clone())
  );
  assert_eq!(
    "2014-11-28T21:00:09+09:00".parse::<DateTime<Utc>>(),
    Ok(dt.clone())
  );
  assert_eq!(
    "2014-11-28T21:00:09+09:00".parse::<DateTime<FixedOffset>>(),
    Ok(fixed_dt.clone())
  );

  // method 2
  assert_eq!(
    DateTime::parse_from_str("2014-11-28 21:00:09 +09:00", "%Y-%m-%d %H:%M:%S %z"),
    Ok(fixed_dt.clone())
  );
  assert_eq!(
    DateTime::parse_from_rfc2822("Fri, 28 Nov 2014 21:00:09 +0900"),
    Ok(fixed_dt.clone())
  );
  assert_eq!(
    DateTime::parse_from_rfc3339("2014-11-28T21:00:09+09:00"),
    Ok(fixed_dt.clone())
  );

  // oops, the year is missing!
  assert!(DateTime::parse_from_str("Fri Nov 28 12:00:09", "%a %b %e %T %Y").is_err());
  // oops, the format string does not include the year at all!
  assert!(DateTime::parse_from_str("Fri Nov 28 12:00:09", "%a %b %e %T").is_err());
  // oops, the weekday is incorrect!
  assert!(DateTime::parse_from_str("Sat Nov 28 12:00:09 2014", "%a %b %e %T %Y").is_err());

  // Construct a datetime from epoch:
  let dt: DateTime<Utc> = DateTime::from_timestamp(1_500_000_000, 0).unwrap();
  assert_eq!(dt.to_rfc2822(), "Fri, 14 Jul 2017 02:40:00 +0000");

  // Create a NaiveDate instance (without time zone)
  let naive_date = NaiveDate::from_ymd_opt(2024, 8, 9).unwrap();

  // Create a NaiveTime instance (without time zone)
  let naive_time = naive_date.and_hms_opt(12, 30, 45).unwrap();

  // Convert NaiveDateTime to DateTime<Utc> and get the timestamp
  let utc_datetime: DateTime<Utc> = naive_time.and_utc();
  let timestamp = utc_datetime.timestamp();

  // Print the NaiveDateTime, DateTime<Utc>, and the timestamp
  println!("NaiveDateTime: {}", naive_time);
  println!("DateTime<Utc>: {}", utc_datetime);
  println!("Timestamp: {}", timestamp);
}

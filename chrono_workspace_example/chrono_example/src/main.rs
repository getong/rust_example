use chrono::{
  offset::FixedOffset, DateTime, Local, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone,
  Utc, Weekday,
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
  naivedatetime_func_example();
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

#[allow(deprecated)]
fn naivedatetime_func_example() {
  let d = NaiveDate::from_ymd_opt(2015, 6, 3).unwrap();
  println!("naive is {}", d);
  assert!(d.and_hms_opt(12, 34, 56).is_some());
  assert!(d.and_hms_opt(12, 34, 60).is_none()); // use `and_hms_milli_opt` instead
  assert!(d.and_hms_opt(12, 60, 56).is_none());
  assert!(d.and_hms_opt(24, 34, 56).is_none());

  let datetime_utc: DateTime<Utc> = Utc::now();
  println!("Original DateTime<Utc>: {}", datetime_utc);

  // Convert DateTime<Utc> to NaiveDateTime (UTC)
  let naive_datetime: NaiveDateTime = datetime_utc.naive_utc();
  println!("Converted NaiveDateTime (UTC): {}", naive_datetime);

  println!("NaiveDateTime timestamp: {}", naive_datetime.timestamp());
  println!(
    "NaiveDateTime utc timestamp: {}",
    naive_datetime.and_utc().timestamp()
  );
  // `chrono::NaiveDateTime::timestamp`: equals `.and_utc().timestamp()`
  assert_eq!(
    naive_datetime.timestamp(),
    naive_datetime.and_utc().timestamp()
  );

  let a_naive_time = NaiveDateTime::from_timestamp_opt(999999, 0).unwrap_or(NaiveDateTime::MIN);

  let b_naive_time = DateTime::from_timestamp(999999, 0)
    .unwrap_or(DateTime::<Utc>::MIN_UTC)
    .naive_utc();

  // `chrono::NaiveDateTime::from_timestamp_opt` equals `DateTime::from_timestamp`
  assert_eq!(a_naive_time, b_naive_time);

  // Convert NaiveDateTime to DateTime<Utc>
  let datetime_utc: DateTime<Utc> = naive_datetime.and_utc();
  println!("Converted DateTime<Utc>: {}", datetime_utc);

  let start = "2024-09-13";
  let end = Some("2024-09-20");

  let start_date = NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap_or(NaiveDate::MIN);
  let start_at = NaiveDateTime::from(start_date);

  let end_date = if let Some(end) = end {
    NaiveDate::parse_from_str(&end, "%Y-%m-%d").unwrap_or(NaiveDate::MIN)
  } else {
    Utc::now().naive_utc().date()
  };
  let end_at = NaiveDateTime::new(end_date, NaiveTime::from_hms_opt(23, 59, 59).unwrap());
  println!("start_at is {:#?}", start_at);
  println!("end_at is {:#?}", end_at);
}

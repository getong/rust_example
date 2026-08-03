use jiff::{Timestamp, ToSpan, Unit, Zoned};

fn main() -> Result<(), jiff::Error> {
  let now = Zoned::now().round(Unit::Second)?;
  println!("{now}");

  let time: Timestamp = "2024-07-11T01:14:00Z".parse()?;
  let zoned = time
    .in_tz("America/New_York")?
    .checked_add(1.month().hours(2))?;
  assert_eq!(
    zoned.to_string(),
    "2024-08-10T23:14:00-04:00[America/New_York]"
  );
  // Or, if you want an RFC3339 formatted string:
  assert_eq!(zoned.timestamp().to_string(), "2024-08-11T03:14:00Z");
  Ok(())
}

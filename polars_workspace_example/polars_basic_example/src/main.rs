use chrono::prelude::*;
use polars::prelude::*;

fn main() {
  let df: DataFrame = df!(
      "integer" => &[1, 2, 3],
      "date" => &[
          NaiveDate::from_ymd_opt(2025, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap(),
          NaiveDate::from_ymd_opt(2025, 1, 2).unwrap().and_hms_opt(0, 0, 0).unwrap(),
          NaiveDate::from_ymd_opt(2025, 1, 3).unwrap().and_hms_opt(0, 0, 0).unwrap(),
      ],
      "float" => &[4.0, 5.0, 6.0],
      "string" => &["a", "b", "c"],
  )
  .unwrap();

  println!("{}", df);
}

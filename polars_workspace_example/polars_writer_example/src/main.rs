use std::fs::File;

use chrono::prelude::*;
use polars::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut df: DataFrame = df!(
    "name" => ["Alice Archer", "Ben Brown", "Chloe Cooper", "Daniel Donovan"],
    "birthdate" => [
      NaiveDate::from_ymd_opt(1997, 1, 10).unwrap(),
      NaiveDate::from_ymd_opt(1985, 2, 15).unwrap(),
      NaiveDate::from_ymd_opt(1983, 3, 22).unwrap(),
      NaiveDate::from_ymd_opt(1981, 4, 30).unwrap(),
    ],
    "weight" => [57.9, 72.5, 53.6, 83.1],  // (kg)
    "height" => [1.56, 1.77, 1.65, 1.75],  // (m)
  )
  .unwrap();
  println!("{df}");

  let mut file = File::create("output.csv").expect("could not create file");
  CsvWriter::new(&mut file)
    .include_header(true)
    .with_separator(b',')
    .finish(&mut df)?;
  let df_csv = CsvReadOptions::default()
    .with_has_header(true)
    .with_parse_options(CsvParseOptions::default().with_try_parse_dates(true))
    .try_into_reader_with_file_path(Some("output.csv".into()))?
    .finish()?;
  println!("{df_csv}");
  Ok(())
}

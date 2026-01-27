use chrono::prelude::*;
use polars::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

  let df: DataFrame = df!(
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

  let result = df
    .clone()
    .lazy()
    .select([
      col("name"),
      col("birthdate").dt().year().alias("birth_year"),
      (col("weight") / col("height").pow(2)).alias("bmi"),
    ])
    .collect()?;
  println!("transform : {result}");

  let result = df
    .clone()
    .lazy()
    .select([
      col("name"),
      (cols(["weight", "height"]).as_expr() * lit(0.95))
        .round(2, RoundMode::default())
        .name()
        .suffix("-5%"),
    ])
    .collect()?;
  println!("transform example2: {result}");

  Ok(())
}

use polars::{lazy::prelude::*, prelude::*};

fn main() -> PolarsResult<()> {
  let df = LazyCsvReader::new("data.csv")
    .with_has_header(true)
    .finish()?;

  let filtered = df.lazy().filter(col("age").gt(lit(30))).collect()?;

  println!("{:?}", filtered);

  Ok(())
}

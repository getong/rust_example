use polars::{lazy::prelude::*, prelude::*};

fn main() -> PolarsResult<()> {
  let df = LazyCsvReader::new("data.csv")
    .with_has_header(true)
    .finish()?;

  let filtered = df.clone().lazy().filter(col("age").gt(lit(30))).collect()?;

  println!("{:?}", filtered);

  let result = df
    .clone()
    .group_by([col("name")])
    .agg([col("score").mean().alias("average_score")])
    .collect()?;

  println!("{:?}", result);

  let result = df
    .clone()
    .select([col("name"), col("score").alias("final_score"), col("age")])
    .drop(["age"])
    .collect()?;

  println!("{:?}", result);

  // Drop rows where name == "Alice"
  let result = df.filter(col("name").neq(lit("Alice"))).collect()?;

  println!("{:?}", result);

  Ok(())
}

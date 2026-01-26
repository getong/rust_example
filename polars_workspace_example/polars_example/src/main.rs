use polars::{lazy::prelude::*, prelude::*};

fn main() -> PolarsResult<()> {
  let df = LazyCsvReader::new(PlPath::new("data.csv"))
    .with_has_header(true)
    .finish()?;

  let filtered = df.clone().filter(col("age").gt(lit(30))).collect()?;

  println!("{:?}", filtered);

  let grouped = df
    .clone()
    .group_by([col("name")])
    .agg([col("score").mean().alias("average_score")])
    .collect()?;

  println!("{:?}", grouped);

  let selected = df
    .clone()
    .select([col("name"), col("score").alias("final_score")])
    .collect()?;

  println!("{:?}", selected);

  // Drop rows where name == "Alice"
  let without_alice = df.filter(col("name").neq(lit("Alice"))).collect()?;

  println!("{:?}", without_alice);

  Ok(())
}

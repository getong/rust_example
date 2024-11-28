use datafusion::{error::Result as DataFusionResult, prelude::*};

#[tokio::main]
async fn main() -> DataFusionResult<()> {
  // Initialize query interface
  let ctx = SessionContext::new();

  // Creates a data frame that describes a query to scan a  CSV file,
  // calculate the average of each score by group,
  // then finally sort by group.
  let df = ctx
    .read_csv("../data/StudentACTResults.csv", CsvReadOptions::new())
    .await?
    .aggregate(
      vec![col("group")],
      vec![
        avg(col("english")),
        avg(col("reading")),
        avg(col("math")),
        avg(col("science")),
      ],
    )?
    .sort(vec![col("group").sort(true, true)])?;

  // Execute the query defined by the data frame
  // and collect the results
  let results = df.collect().await?;

  println!("{:?}", results);

  Ok(())
}

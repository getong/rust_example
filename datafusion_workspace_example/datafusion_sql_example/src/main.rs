use datafusion::{error::Result as DataFusionResult, prelude::*};

#[tokio::main]
async fn main() -> DataFusionResult<()> {
  // Initialize query interface
  let ctx = SessionContext::new();

  // Register data source in the execution context
  // with a given name
  ctx
    .register_csv(
      "scores",
      "../data/StudentACTResults.csv",
      CsvReadOptions::new(),
    )
    .await?;

  // Create a data frame with a with a query defined by
  // a SQL statement
  let df = ctx
    .sql(
      "SELECT group, AVG(english), AVG(reading), AVG(math), AVG(science)
         FROM scores
         GROUP BY group
         ORDER BY group",
    )
    .await?;

  // Execute the query defined by the data frame
  // and collect the results
  let results = df.collect().await?;

  println!("{:?}", results);

  Ok(())
}

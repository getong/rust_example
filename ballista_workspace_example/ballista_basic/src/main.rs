use ballista::datafusion::prelude::*;

#[tokio::main]
async fn main() -> ballista::datafusion::error::Result<()> {
  // create SessionContext
  let ctx = SessionContext::new();

  // register the table
  ctx
    .register_csv("example", "example.csv", CsvReadOptions::new())
    .await?;

  // create a plan to run a SQL query
  let df = ctx
    .sql("SELECT a, MIN(b) FROM example WHERE a <= b GROUP BY a LIMIT 100")
    .await?;

  // execute and print results
  df.show().await?;
  Ok(())
}

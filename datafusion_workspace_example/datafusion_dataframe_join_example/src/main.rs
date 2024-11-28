use datafusion::{error::Result as DataFusionResult, prelude::*};

#[tokio::main]
async fn main() -> DataFusionResult<()> {
  // Initialize query interface
  let ctx = SessionContext::new();

  // Create data frame from student scores
  let scores_df = ctx
    .read_csv("../data/StudentACTResults.csv", CsvReadOptions::new())
    .await?;

  // Create data from students
  let students_df = ctx
    .read_csv("../data/Students.csv", CsvReadOptions::new())
    .await?;

  // Create a data frame that represents an inner join
  // on both data frames
  let join = students_df.join(scores_df, JoinType::Inner, &["student_id"], &["student"])?;

  // Collect results
  let results = join.collect().await?;

  println!("{:?}", results);

  Ok(())
}

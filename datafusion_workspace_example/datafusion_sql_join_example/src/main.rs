use datafusion::error::Result as DataFusionResult;
use datafusion::prelude::*;

#[tokio::main]
async fn main() -> DataFusionResult<()> {
    // Initialize query interface
    let ctx = SessionContext::new();

    // Register scores data source in the execution context
    ctx.register_csv(
        "scores",
        "../data/StudentACTResults.csv",
        CsvReadOptions::new(),
    )
    .await?;

    // Register students in the exectuion context
    ctx.register_csv("students", "../data/Students.csv", CsvReadOptions::new())
        .await?;

    // Create a data frame with a with a query defined by
    // a SQL statement to join both data sources
    let df = ctx
        .sql(
            "SELECT *
         FROM students
         JOIN scores ON student_id = student",
        )
        .await?;

    // Execute the query defined by the data frame
    // and collect the results
    let results = df.collect().await?;

    println!("{:?}", results);

    Ok(())
}

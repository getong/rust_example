use std::fs::File;
use std::sync::Arc;

use arrow::csv;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::error::Result as ArrowResult;

fn main() -> ArrowResult<()> {
    let _file = File::open("../../data/StudentACTResults.csv").unwrap();

    reader_example()?;
    reader_builder_example()?;

    Ok(())
}

fn reader_example() -> ArrowResult<()> {
    // Initialize schema
    let schema = Schema::new(vec![
        Field::new("student", DataType::Int64, false),
        Field::new("attended_study_group", DataType::Boolean, false),
        Field::new("group", DataType::Int64, false),
        Field::new("english", DataType::Int64, false),
        Field::new("reading", DataType::Int64, false),
        Field::new("math", DataType::Int64, false),
        Field::new("science", DataType::Int64, false),
    ]);

    let file = File::open("../../data/StudentACTResults.csv").unwrap();

    // Initialize reader with indicator that headers exits and batch size
    let mut csv_reader =
        csv::Reader::new(file, Arc::new(schema), true, None, 1000, None, None, None);

    // Get next batch record
    let batch = csv_reader.next().unwrap().unwrap();

    println!("{:?}", batch);

    Ok(())
}

fn reader_builder_example() -> ArrowResult<()> {
    let file = File::open("../../data/StudentACTResults.csv").unwrap();

    // Configure CSV builder that infers
    // the schema of the CSV file
    let csv_builder = csv::ReaderBuilder::new()
        .has_header(true)
        .infer_schema(Some(100));

    // Build CSV reader
    let mut csv_reader = csv_builder.build(file)?;

    // Get next batch record from CSV reader
    // CSV reader is an iterator over RecordBatch results
    let batch = csv_reader.next().unwrap().unwrap();

    println!("{:?}", batch);

    Ok(())
}

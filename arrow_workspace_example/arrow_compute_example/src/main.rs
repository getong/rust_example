use std::fs::File;
use std::sync::Arc;

use arrow::array::{ArrayRef, BooleanArray, Float64Array, Int64Array};
use arrow::compute::{filter, sort, sort_to_indices, sum, take};
use arrow::csv;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::error::Result as ArrowResult;
use arrow::record_batch::RecordBatch;

fn main() -> ArrowResult<()> {
  let file = File::open("../../data/StudentACTResults.csv").unwrap();

  // Configure CSV builder
  let csv_builder = csv::ReaderBuilder::new()
    .has_header(true)
    .infer_schema(Some(100));

  // Build CSV reader
  let mut csv_reader = csv_builder.build(file)?;

  // Get next batch record from CSV reader
  // CSV reader is an iterator over RecordBatch results
  let batch = csv_reader.next().unwrap().unwrap();

  // Sort batch by group
  let sorted_batch = sort_by_group(&batch).unwrap();

  println!("{:?}", sorted_batch);

  // Filter out all results except group 1
  let filtered_batch = filter_by_group(1, &batch)?;

  println!("{:?}", filtered_batch);

  // Average score by group
  let averaged_batch = average_score_by_group(&batch)?;

  println!("{:?}", averaged_batch);

  Ok(())
}

fn sort_by_group(batch: &RecordBatch) -> ArrowResult<RecordBatch> {
  // Build an array of sorted indices
  let indices = sort_to_indices(batch.column(2), None, None)?;

  // Create a new RecordBatch with re-ordered
  // rows from the original batch by calling
  // take on each column
  RecordBatch::try_new(
    batch.schema(),
    batch
      .columns()
      .iter()
      .map(|column| take(column.as_ref(), &indices, None))
      .collect::<ArrowResult<Vec<ArrayRef>>>()?,
  )
}

fn filter_by_group(group: i64, batch: &RecordBatch) -> ArrowResult<RecordBatch> {
  // Create a boolean array that determines which values are filtered out
  let filter_array = batch
    .column(2)
    .as_any()
    .downcast_ref::<Int64Array>()
    .unwrap()
    .iter()
    .map(|value| Some(value == Some(group)))
    .collect::<BooleanArray>();

  let mut arrays: Vec<ArrayRef> = Vec::new();

  // Iterate over the columns and apply filter
  for idx in 0 .. batch.num_columns() {
    let array = batch.column(idx).as_ref();

    // Apply filter to column;
    let filtered = filter(array, &filter_array)?;

    arrays.push(filtered);
  }

  // Create a new record batch from filtered results
  RecordBatch::try_new(batch.schema(), arrays)
}

fn average_score_by_group(batch: &RecordBatch) -> ArrowResult<RecordBatch> {
  // Find unique instances of the group column
  let mut groups = sort(batch.column(2), None)?
    .as_any()
    .downcast_ref::<Int64Array>()
    .unwrap()
    .values()
    .to_vec();

  groups.dedup();

  // Initialize builders
  let mut builders = vec![
    Float64Array::builder(groups.len()),
    Float64Array::builder(groups.len()),
    Float64Array::builder(groups.len()),
    Float64Array::builder(groups.len()),
  ];

  // For each unique group
  // Calculate the average for each score column
  for group in &groups {
    let mut builder_idx = 0;
    let group_batch = filter_by_group(*group, &batch)?;

    let row_count = group_batch.num_rows() as f64;

    for col_idx in 3 ..= 6 {
      let column = group_batch
        .column(col_idx)
        .as_any()
        .downcast_ref::<Int64Array>()
        .unwrap();

      let column_sum = sum(column).unwrap() as f64;

      builders[builder_idx].append_value(column_sum / row_count)?;

      builder_idx += 1;
    }
  }

  // Compile results from builder arrays
  let mut results: Vec<ArrayRef> = vec![Arc::new(Int64Array::from(groups))];
  for mut builder in builders {
    results.push(Arc::new(builder.finish()));
  }

  // Initialize new schema to reflect aggregation of original batch
  let schema = Schema::new(vec![
    Field::new("group", DataType::Int64, false),
    Field::new("english", DataType::Float64, false),
    Field::new("reading", DataType::Float64, false),
    Field::new("math", DataType::Float64, false),
    Field::new("science", DataType::Float64, false),
  ]);

  RecordBatch::try_new(Arc::new(schema), results)
}

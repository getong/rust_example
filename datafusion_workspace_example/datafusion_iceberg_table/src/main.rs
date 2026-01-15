use std::{
  fs::{File, create_dir_all},
  sync::Arc,
};

use datafusion::{
  arrow::{
    array::{Int32Array, StringArray, TimestampMillisecondArray},
    datatypes::{DataType, Field, Schema, TimeUnit},
    record_batch::RecordBatch,
  },
  prelude::*,
};
use parquet::{arrow::ArrowWriter, file::properties::WriterProperties};

fn generate_test_data(output_path: &str) -> datafusion::error::Result<()> {
  // Create directory if it doesn't exist
  if let Some(parent) = std::path::Path::new(output_path).parent() {
    create_dir_all(parent)?;
  }

  // Define schema
  let schema = Schema::new(vec![
    Field::new(
      "timestamp",
      DataType::Timestamp(TimeUnit::Millisecond, None),
      false,
    ),
    Field::new("user_id", DataType::Utf8, false),
    Field::new("response_time_ms", DataType::Int32, false),
    Field::new("status_code", DataType::Int32, false),
  ]);

  // Generate sample data
  let num_records = 1000;
  let now = chrono::Utc::now().timestamp_millis();
  let one_hour_ms = 3600 * 1000;

  let mut timestamps = Vec::new();
  let mut user_ids = Vec::new();
  let mut response_times = Vec::new();
  let mut status_codes = Vec::new();

  for i in 0 .. num_records {
    // Generate timestamps from last 24 hours
    timestamps.push(now - (i % 24) * one_hour_ms - (i * 1000));
    user_ids.push(format!("user_{}", i % 100));
    response_times.push((50 + (i * 7) % 500) as i32);
    status_codes.push(if i % 50 == 0 { 500 } else { 200 });
  }

  // Create arrays
  let timestamp_array = TimestampMillisecondArray::from(timestamps);
  let user_id_array = StringArray::from(user_ids);
  let response_time_array = Int32Array::from(response_times);
  let status_code_array = Int32Array::from(status_codes);

  // Create record batch
  let batch = RecordBatch::try_new(
    Arc::new(schema.clone()),
    vec![
      Arc::new(timestamp_array),
      Arc::new(user_id_array),
      Arc::new(response_time_array),
      Arc::new(status_code_array),
    ],
  )?;

  // Write to parquet file
  let file = File::create(output_path)?;
  let props = WriterProperties::builder().build();
  let mut writer = ArrowWriter::try_new(file, Arc::new(schema), Some(props))?;
  writer.write(&batch)?;
  writer.close()?;

  println!("Generated {} test records in {}", num_records, output_path);
  Ok(())
}

async fn process_logs_batch(input_path: &str, output_path: &str) -> datafusion::error::Result<()> {
  let ctx = SessionContext::new();

  // Register input data
  ctx
    .register_parquet("raw_logs", input_path, ParquetReadOptions::default())
    .await?;

  // Complex transformation using DataFusion SQL
  let df = ctx
    .sql(
      "
        SELECT
            date_trunc('hour', timestamp) as hour,
            user_id,
            COUNT(*) as request_count,
            AVG(response_time_ms) as avg_response_time,
            PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY response_time_ms) as p95_response_time,
            SUM(CASE WHEN status_code >= 500 THEN 1 ELSE 0 END) as error_count
        FROM raw_logs
        WHERE timestamp >= CURRENT_DATE - INTERVAL '1' DAY
        GROUP BY date_trunc('hour', timestamp), user_id
    ",
    )
    .await?;

  // Write to Iceberg
  let batches = df.collect().await?;
  write_iceberg_table(batches, output_path)?;

  Ok(())
}

fn write_iceberg_table(
  batches: Vec<RecordBatch>,
  output_path: &str,
) -> datafusion::error::Result<()> {
  // TODO: Implement Iceberg table writing logic
  println!("Writing {} batches to {}", batches.len(), output_path);
  Ok(())
}

#[tokio::main]
async fn main() -> datafusion::error::Result<()> {
  println!("Processing logs...");

  // Example paths - adjust as needed
  let input_path = "input/logs.parquet";
  let output_path = "output/iceberg_table";

  // Generate test data first
  println!("Generating test data...");
  generate_test_data(input_path)?;

  process_logs_batch(input_path, output_path).await?;

  println!("Done!");
  Ok(())
}

// copy from https://dev.to/mayu2008/building-high-performance-analytics-with-rust-apache-iceberg-and-apache-doris-a-modern-data-stack-agm

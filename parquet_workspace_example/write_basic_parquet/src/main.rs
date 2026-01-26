use std::sync::Arc;

use arrow_array::{Array, Int64Array, RecordBatch, StringArray};
use bytes::Bytes;
use futures::TryStreamExt;
use parquet::{
  arrow::{async_reader::ParquetRecordBatchStreamBuilder, async_writer::AsyncArrowWriter},
  file::properties::WriterProperties,
};
use tokio::{fs::File, io::BufWriter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  // exmaple 1, write in memory
  write_to_memory().await?;

  // example 2, write in file
  write_to_file().await?;

  // exmaple 3, write in complex file
  write_complex_data().await?;

  // example 4, read simple file
  read_simple_file().await?;

  // example 5, read complex file
  read_complex_file().await?;

  Ok(())
}

async fn write_to_memory() -> anyhow::Result<()> {
  let col = Arc::new(Int64Array::from_iter_values([1, 2, 3])) as _;
  let batch = RecordBatch::try_from_iter([("col", col)])?;

  let mut buffer = Vec::new();
  let props = Some(WriterProperties::builder().build());

  let mut writer = AsyncArrowWriter::try_new(&mut buffer, batch.schema(), props)?;
  writer.write(&batch).await?;
  writer.close().await?;

  let bytes = Bytes::from(buffer);
  println!("total bytes: {}", bytes.len());

  Ok(())
}

async fn write_to_file() -> anyhow::Result<()> {
  let col = Arc::new(Int64Array::from_iter_values([10, 20, 30, 40, 50])) as _;
  let batch = RecordBatch::try_from_iter([("numbers", col)])?;

  let file_path = "output_simple.parquet";
  let file = File::create(file_path).await?;
  let writer_buf = BufWriter::new(file);

  let props = Some(WriterProperties::builder().build());
  let mut writer = AsyncArrowWriter::try_new(writer_buf, batch.schema(), props)?;

  writer.write(&batch).await?;
  writer.close().await?;

  println!("data has written into : {}", file_path);

  Ok(())
}

async fn write_complex_data() -> anyhow::Result<()> {
  println!("\n=== example 3");

  let id_array = Arc::new(Int64Array::from(vec![1, 2, 3, 4, 5])) as _;
  let name_array = Arc::new(StringArray::from(vec![
    "Alice", "Bob", "Charlie", "David", "Eve",
  ])) as _;
  let age_array = Arc::new(Int64Array::from(vec![
    Some(25),
    None,
    Some(30),
    Some(35),
    Some(28),
  ])) as _;

  let batch =
    RecordBatch::try_from_iter([("id", id_array), ("name", name_array), ("age", age_array)])?;

  let file_path = "output_complex.parquet";
  let file = File::create(file_path).await?;
  let writer_buf = BufWriter::new(file);

  let props = Some(
    WriterProperties::builder()
      .set_compression(parquet::basic::Compression::SNAPPY)
      .build(),
  );

  let mut writer = AsyncArrowWriter::try_new(writer_buf, batch.schema(), props)?;

  writer.write(&batch).await?;
  writer.close().await?;

  println!("comple data written: {}", file_path);
  println!("  - {} arrows", batch.num_rows());
  println!("  - {} columns", batch.num_columns());

  Ok(())
}

async fn read_simple_file() -> anyhow::Result<()> {
  println!("\n=== example 4: read simple file ===");

  let file_path = "output_simple.parquet";
  let file = File::open(file_path).await?;

  let builder = ParquetRecordBatchStreamBuilder::new(file).await?;

  println!("File metadata:");
  println!("  - Schema: {:?}", builder.schema());
  println!(
    "  - Num row groups: {}",
    builder.metadata().num_row_groups()
  );

  let mut stream = builder.build()?;

  println!("\nReading data:");
  while let Some(batch) = stream.try_next().await? {
    println!("  Batch with {} rows:", batch.num_rows());
    for i in 0 .. batch.num_rows() {
      let col = batch.column(0);
      let array = col.as_any().downcast_ref::<Int64Array>().unwrap();
      println!("    Row {}: numbers = {}", i, array.value(i));
    }
  }

  Ok(())
}

async fn read_complex_file() -> anyhow::Result<()> {
  println!("\n=== example 5: read complex file ===");

  let file_path = "output_complex.parquet";
  let file = File::open(file_path).await?;

  let builder = ParquetRecordBatchStreamBuilder::new(file).await?;

  println!("File metadata:");
  println!("  - Schema: {:?}", builder.schema());
  println!(
    "  - Num row groups: {}",
    builder.metadata().num_row_groups()
  );
  println!(
    "  - Total rows: {:?}",
    builder.metadata().file_metadata().num_rows()
  );

  let mut stream = builder.build()?;

  println!("\nReading data:");
  while let Some(batch) = stream.try_next().await? {
    println!("  Batch with {} rows:", batch.num_rows());

    let id_col = batch
      .column(0)
      .as_any()
      .downcast_ref::<Int64Array>()
      .unwrap();
    let name_col = batch
      .column(1)
      .as_any()
      .downcast_ref::<StringArray>()
      .unwrap();
    let age_col = batch
      .column(2)
      .as_any()
      .downcast_ref::<Int64Array>()
      .unwrap();

    for i in 0 .. batch.num_rows() {
      let id = id_col.value(i);
      let name = name_col.value(i);
      let age = if age_col.is_null(i) {
        "None".to_string()
      } else {
        age_col.value(i).to_string()
      };
      println!("    Row {}: id={}, name={}, age={}", i, id, name, age);
    }
  }

  Ok(())
}

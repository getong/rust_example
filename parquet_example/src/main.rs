use std::{fs::File, sync::Arc};

use arrow::{
  array::{Float64Array, StringArray},
  datatypes::{DataType, Field, Schema},
  record_batch::RecordBatch,
};
use parquet::{arrow::ArrowWriter, file::properties::WriterProperties};
use rdkafka::{
  ClientConfig, Message,
  consumer::{Consumer, StreamConsumer},
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Event {
  id: String,
  value: f64,
}

async fn stream_to_iceberg(
  kafka_brokers: &str,
  topic: &str,
  iceberg_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
  let consumer: StreamConsumer = ClientConfig::new()
    .set("bootstrap.servers", kafka_brokers)
    .set("group.id", "analytics-pipeline")
    .create()?;

  consumer.subscribe(&[topic])?;

  let (tx, mut rx) = mpsc::channel(1000);

  // Consumption task
  tokio::spawn(async move {
    loop {
      match consumer.recv().await {
        Ok(message) => {
          if let Some(payload) = message.payload() {
            match serde_json::from_slice::<Event>(payload) {
              Ok(event) => {
                if let Err(e) = tx.send(event).await {
                  eprintln!("Failed to send event: {}", e);
                  break;
                }
              }
              Err(e) => eprintln!("Failed to deserialize event: {}", e),
            }
          }
        }
        Err(e) => eprintln!("Kafka error: {}", e),
      }
    }
  });

  // Batch writing task
  let mut batch_buffer = Vec::new();
  let batch_size = 10000;

  while let Some(event) = rx.recv().await {
    batch_buffer.push(event);

    if batch_buffer.len() >= batch_size {
      let record_batch = create_record_batch(&batch_buffer)?;
      write_to_iceberg(record_batch, iceberg_path).await?;
      batch_buffer.clear();
    }
  }

  Ok(())
}

fn create_record_batch(events: &[Event]) -> Result<RecordBatch, Box<dyn std::error::Error>> {
  let schema = Schema::new(vec![
    Field::new("id", DataType::Utf8, false),
    Field::new("value", DataType::Float64, false),
  ]);

  let ids: StringArray = events.iter().map(|e| Some(e.id.as_str())).collect();
  let values: Float64Array = events.iter().map(|e| Some(e.value)).collect();

  let batch = RecordBatch::try_new(Arc::new(schema), vec![Arc::new(ids), Arc::new(values)])?;

  Ok(batch)
}

async fn write_to_iceberg(
  batch: RecordBatch,
  path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
  let file = File::create(path)?;
  let props = WriterProperties::builder().build();
  let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props))?;

  writer.write(&batch)?;
  writer.close()?;

  Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  println!("Starting Kafka to Iceberg streaming...");

  let kafka_brokers = "localhost:9092";
  let topic = "events";
  let iceberg_path = "output.parquet";

  stream_to_iceberg(kafka_brokers, topic, iceberg_path).await?;

  Ok(())
}

// copy from https://dev.to/mayu2008/building-high-performance-analytics-with-rust-apache-iceberg-and-apache-doris-a-modern-data-stack-agm
use std::time::Duration;

use fluvio::{Offset, RecordKey};
use futures::StreamExt;

const TOPIC: &str = "echo-test";
const MAX_RECORDS: u8 = 10;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let producer = fluvio::producer(TOPIC).await?;
    let consumer = fluvio::consumer(TOPIC, 0).await?;
  let mut consumed_records: u8 = 0;

  for i in 0 .. 10 {
    producer
      .send(RecordKey::NULL, format!("Hello from Fluvio {}!", i))
      .await?;
    println!("[PRODUCER] sent record {}", i);
    tokio::time::sleep(Duration::from_secs(1)).await;
  }

  // Fluvio batches records by default, call flush() when done producing
  // to ensure all records are sent
  producer.flush().await?;

  let mut stream = consumer.stream(Offset::beginning()).await?;

  while let Some(Ok(record)) = stream.next().await {
    let value_str = record.get_value().as_utf8_lossy_string();

    println!("[CONSUMER] Got record: {}", value_str);
    consumed_records += 1;

    if consumed_records >= MAX_RECORDS {
      break;
    }
  }

  Ok(())
}

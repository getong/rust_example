use std::time::Duration;

use fluvio::{
  Fluvio, FluvioClusterConfig, Offset, RecordKey,
  config::{Config, ConfigFile, Profile},
  consumer::ConsumerConfigExtBuilder,
};
use futures::StreamExt;

const TOPIC: &str = "echo-test";
const MAX_RECORDS: u8 = 10;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // let fluvio = Fluvio::connect().await?;
  let mut config = Config::new();
  let cluster = FluvioClusterConfig::new("https://cloud.fluvio.io".to_string());
  config.add_cluster(cluster, "fluvio-cloud".to_string());
  let profile = Profile::new("fluvio-cloud".to_string());
  config.add_profile(profile, "fluvio-cloud".to_string());
  config.set_current_profile("fluvio-cloud");

  let mut config_file = ConfigFile::load_default_or_new()?;
  *config_file.mut_config() = config;
  let cluster_config = config_file.config().current_cluster()?;
  let fluvio = Fluvio::connect_with_config(cluster_config).await?;
  let producer = fluvio.topic_producer(TOPIC).await?;

  for i in 0 .. MAX_RECORDS {
    producer
      .send(RecordKey::NULL, format!("Hello from Fluvio {}!", i))
      .await?;
    println!("[PRODUCER] sent record {}", i);
    tokio::time::sleep(Duration::from_secs(1)).await;
  }

  // Fluvio batches records by default, call flush() when done producing
  // to ensure all records are sent
  producer.flush().await?;

  let consumer_config = ConsumerConfigExtBuilder::default()
    .topic(TOPIC.to_string())
    .partition(0)
    .offset_start(Offset::beginning())
    .build()?;
  let mut stream = fluvio.consumer_with_config(consumer_config).await?;

  let mut consumed_records: u8 = 0;
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

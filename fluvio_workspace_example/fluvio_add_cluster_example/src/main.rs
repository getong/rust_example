use std::time::Duration;

use fluvio::{
  Fluvio, FluvioAdmin, FluvioClusterConfig, Offset, RecordKey,
  config::{ConfigFile, Profile},
  consumer::ConsumerConfigExtBuilder,
  metadata::topic::TopicSpec,
};
use futures::StreamExt;

const TOPIC: &str = "echo-test";
const MAX_RECORDS: u8 = 10;
const CLUSTER_NAME: &str = "fluvio-cloud";
const CLUSTER_NAME_ALT: &str = "fluvio-cloud-backup";
const CLUSTER_NAME_LOCAL: &str = "fluvio-local";
const DOCKER_SC_ENDPOINT: &str = "127.0.0.1:9103";
const DOCKER_SC_ENDPOINT_ALT: &str = "127.0.0.1:9104";
const DOCKER_SC_ENDPOINT_LOCAL: &str = "localhost:9003";

async fn ensure_topic(admin: &FluvioAdmin, topic: &str) -> Result<(), Box<dyn std::error::Error>> {
  let topics = admin.list::<TopicSpec, _>(Vec::<String>::new()).await?;
  let exists = topics.iter().any(|existing| existing.name == topic);

  if !exists {
    let topic_spec = TopicSpec::new_computed(1, 1, None);
    admin.create(topic.to_string(), false, topic_spec).await?;
    println!("[ADMIN] created topic {}", topic);
  }

  Ok(())
}

/// Adds or updates a cluster profile similar to running `fluvio cluster add`.
fn add_cluster_to_config(
  cluster_name: &str,
  endpoint: &str,
) -> Result<FluvioClusterConfig, Box<dyn std::error::Error>> {
  let mut config_file = ConfigFile::load_default_or_new()?;
  let cluster_config = FluvioClusterConfig::new(endpoint.to_string());
  let config = config_file.mut_config();

  config.add_cluster(cluster_config.clone(), cluster_name.to_string());

  let profile = config
    .profile
    .entry(cluster_name.to_string())
    .or_insert_with(|| Profile::new(cluster_name.to_string()));
  profile.set_cluster(cluster_name.to_string());

  if !config.set_current_profile(cluster_name) {
    return Err(format!("Failed to set current profile {cluster_name}").into());
  }

  config_file.save()?;
  println!("[CONFIG] added cluster {cluster_name} -> {endpoint}");

  Ok(cluster_config)
}

/// Prints clusters in the saved config similar to `fluvio cluster list`.
fn print_cluster_list() -> Result<(), Box<dyn std::error::Error>> {
  let config_file = ConfigFile::load_default_or_new()?;
  let config = config_file.config();

  println!(
    "[CONFIG] current profile: {}",
    config.current_profile_name().unwrap_or("None")
  );

  if config.cluster.is_empty() {
    println!("[CONFIG] no clusters configured");
  } else {
    println!("[CONFIG] clusters:");
    for (name, cluster) in &config.cluster {
      println!("  - {name}: {}", cluster.endpoint);
    }
  }

  Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Connect to the docker-compose SC (container 9003 mapped to host 9103)
  let clusters = [
    (CLUSTER_NAME, DOCKER_SC_ENDPOINT),
    (CLUSTER_NAME_ALT, DOCKER_SC_ENDPOINT_ALT),
    (CLUSTER_NAME_LOCAL, DOCKER_SC_ENDPOINT_LOCAL),
  ];

  let mut primary_cluster = None;
  for (idx, (name, endpoint)) in clusters.iter().enumerate() {
    let cfg = add_cluster_to_config(name, endpoint)?;
    if idx == 0 {
      primary_cluster = Some(cfg);
    }
  }

  let cluster_config = primary_cluster.expect("primary cluster config should exist");
  print_cluster_list()?;

  let fluvio = Fluvio::connect_with_config(&cluster_config).await?;
  let admin = fluvio.admin().await;
  ensure_topic(&admin, TOPIC).await?;
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

// docker cluster
// read https://www.fluvio.io/docs/fluvio/installation/docker
// https://github.com/fluvio-community/fluvio/tree/master/examples/docker-compose

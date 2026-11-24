use std::{
  collections::{HashMap, HashSet},
  error::Error,
  io,
  process::ExitCode,
  time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use confluent_kafka::{
  common::{
    Error as KafkaError,
    serialization::{StringDeserializer, StringSerializer},
  },
  consumer::{Consumer, ConsumerConfig, GroupProtocol, KafkaConsumer},
  producer::{KafkaProducer, Producer, ProducerConfig, ProducerRecord},
};

mod pool;

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone)]
struct Settings {
  brokers: String,
  topic: String,
  group: String,
  count: usize,
  timeout: Duration,
}

impl Settings {
  fn from_env() -> AppResult<Self> {
    let count = env_or("KAFKA_MESSAGE_COUNT", "10").parse::<usize>()?;
    let timeout = env_or("KAFKA_TIMEOUT_SECONDS", "60").parse::<u64>()?;
    if count == 0 || count > i32::MAX as usize || timeout == 0 {
      return Err(
        io::Error::new(
          io::ErrorKind::InvalidInput,
          "KAFKA_MESSAGE_COUNT must be 1..=2147483647; KAFKA_TIMEOUT_SECONDS must be positive",
        )
        .into(),
      );
    }
    Ok(Self {
      brokers: env_or("KAFKA_BOOTSTRAP_SERVERS", "localhost:9092"),
      topic: env_or("KAFKA_TOPIC", "confluent-rust-demo"),
      group: env_or("KAFKA_GROUP_ID", "confluent-rust-example"),
      count,
      timeout: Duration::from_secs(timeout),
    })
  }
}

fn main() -> ExitCode {
  let result = (|| -> AppResult<()> {
    let threads = positive_env("KAFKA_WORKER_THREADS", 4)?;
    tokio::runtime::Builder::new_multi_thread()
      .worker_threads(threads)
      .thread_name("kafka-worker")
      .enable_all()
      .build()?
      .block_on(run())
  })();
  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("Error: {error}");
      ExitCode::FAILURE
    }
  }
}

async fn run() -> AppResult<()> {
  let arguments: Vec<String> = std::env::args().skip(1).collect();
  let command = arguments.first().map(String::as_str).unwrap_or("demo");
  if arguments.len() > 1
    || !matches!(
      command,
      "demo" | "produce" | "consume" | "pool" | "broadcast" | "--help" | "-h"
    )
  {
    return Err(
      io::Error::new(
        io::ErrorKind::InvalidInput,
        "Usage: confluent_kafka_example [demo|produce|consume|pool|broadcast|--help]",
      )
      .into(),
    );
  }
  if matches!(command, "--help" | "-h") {
    println!(
      "Usage: confluent_kafka_example [demo|produce|consume|pool|broadcast]\nEnvironment: \
       KAFKA_BOOTSTRAP_SERVERS, KAFKA_TOPIC, KAFKA_GROUP_ID,\nKAFKA_MESSAGE_COUNT (10), \
       KAFKA_TIMEOUT_SECONDS (60), KAFKA_CLIENTS (3), KAFKA_WORKER_THREADS (4)"
    );
    return Ok(());
  }

  let mut settings = Settings::from_env()?;
  let run_id = format!(
    "{}-{}",
    std::process::id(),
    SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
  );
  println!("brokers={}, topic={}", settings.brokers, settings.topic);
  match command {
    "produce" => produce(&settings, &run_id).await,
    "consume" => consume(&settings, None).await,
    "pool" | "broadcast" => pool::run(settings, &run_id, command == "broadcast").await,
    _ => {
      settings.group = format!("{}-{run_id}", settings.group);
      produce(&settings, &run_id).await?;
      consume(&settings, Some(&run_id)).await?;
      println!(
        "Demo verified: {} messages produced and consumed.",
        settings.count
      );
      Ok(())
    }
  }
}

fn create_producer(settings: &Settings) -> AppResult<KafkaProducer<String, String>> {
  let properties = HashMap::from([
    (
      ProducerConfig::BOOTSTRAP_SERVERS_CONFIG.to_owned(),
      settings.brokers.clone(),
    ),
    (
      ProducerConfig::CLIENT_ID_CONFIG.to_owned(),
      "confluent-rust-producer".to_owned(),
    ),
    (ProducerConfig::ACKS_CONFIG.to_owned(), "all".to_owned()),
    (
      ProducerConfig::ENABLE_IDEMPOTENCE_CONFIG.to_owned(),
      "true".to_owned(),
    ),
    (
      ProducerConfig::COMPRESSION_TYPE_CONFIG.to_owned(),
      "lz4".to_owned(),
    ),
    (
      ProducerConfig::MAX_BLOCK_MS_CONFIG.to_owned(),
      "10000".to_owned(),
    ),
    (
      ProducerConfig::DELIVERY_TIMEOUT_MS_CONFIG.to_owned(),
      "30000".to_owned(),
    ),
    (
      ProducerConfig::REQUEST_TIMEOUT_MS_CONFIG.to_owned(),
      "10000".to_owned(),
    ),
  ]);
  Ok(KafkaProducer::<String, String>::new(
    ProducerConfig::new(&properties)?,
    StringSerializer::new(),
    StringSerializer::new(),
  )?)
}

async fn produce(settings: &Settings, run_id: &str) -> AppResult<()> {
  let producer = create_producer(settings)?;
  let result = async {
    for index in 0 .. settings.count {
      let key = format!("{run_id}:{index}");
      let value = format!("Hello Kafka from Rust #{index}");
      let record = ProducerRecord::with_key(settings.topic.clone(), Some(key), Some(value));
      let metadata = producer.send(record).await?.get().await?;
      println!(
        "Produced: topic={}, partition={}, offset={}",
        metadata.topic(),
        metadata.partition(),
        metadata.offset()
      );
    }
    Ok(())
  }
  .await;
  let close_result = producer.close().await.map_err(Into::into);
  result.and(close_result)
}

fn create_consumer(
  settings: &Settings,
  client_id: &str,
) -> AppResult<KafkaConsumer<String, String>> {
  let properties = HashMap::from([
    (
      ConsumerConfig::BOOTSTRAP_SERVERS_CONFIG.to_owned(),
      settings.brokers.clone(),
    ),
    (
      ConsumerConfig::GROUP_ID_CONFIG.to_owned(),
      settings.group.clone(),
    ),
    (
      ConsumerConfig::CLIENT_ID_CONFIG.to_owned(),
      client_id.to_owned(),
    ),
    (
      ConsumerConfig::GROUP_PROTOCOL_CONFIG.to_owned(),
      GroupProtocol::Consumer.to_string(),
    ),
    (
      ConsumerConfig::AUTO_OFFSET_RESET_CONFIG.to_owned(),
      "earliest".to_owned(),
    ),
    (
      ConsumerConfig::ENABLE_AUTO_COMMIT_CONFIG.to_owned(),
      "false".to_owned(),
    ),
    (
      ConsumerConfig::MAX_POLL_RECORDS_CONFIG.to_owned(),
      settings.count.to_string(),
    ),
  ]);
  Ok(KafkaConsumer::new(
    ConsumerConfig::new(&properties)?,
    StringDeserializer::new(),
    StringDeserializer::new(),
  )?)
}

async fn consume(settings: &Settings, run_id: Option<&str>) -> AppResult<()> {
  let mut consumer = create_consumer(settings, "confluent-rust-consumer")?;
  let handle = consumer.handle();
  let shutdown = tokio::spawn(async move {
    if tokio::signal::ctrl_c().await.is_ok() {
      handle.wakeup();
    }
  });
  let result = async {
    consumer
      .subscribe_with_topics(vec![settings.topic.clone()])
      .await?;
    println!(
      "Consuming: group={}, expected={}",
      settings.group, settings.count
    );
    let started = Instant::now();
    let mut received = 0;
    let mut expected: HashSet<String> = (0 .. settings.count)
      .filter_map(|index| run_id.map(|run_id| format!("{run_id}:{index}")))
      .collect();
    while received < settings.count {
      let remaining = settings.timeout.saturating_sub(started.elapsed());
      if remaining.is_zero() {
        return Err(
          io::Error::new(
            io::ErrorKind::TimedOut,
            format!(
              "received {received}/{} messages before timeout",
              settings.count
            ),
          )
          .into(),
        );
      }
      let records = match consumer.poll(remaining.min(Duration::from_secs(1))).await {
        Ok(records) => records,
        Err(KafkaError::Wakeup(_)) if run_id.is_none() => return Ok(()),
        Err(error) if error.is_retriable_error() => {
          eprintln!("Transient poll error: {error}");
          continue;
        }
        Err(error) => return Err(error.into()),
      };
      if records.is_empty() {
        continue;
      }
      for record in records {
        if run_id.is_some() {
          let Some(key) = record.key().filter(|key| expected.contains(*key)) else {
            continue;
          };
          let index = key.rsplit(':').next().unwrap_or_default();
          let expected_value = format!("Hello Kafka from Rust #{index}");
          if record.value() != Some(&expected_value) {
            return Err(
              io::Error::new(io::ErrorKind::InvalidData, "unexpected message value").into(),
            );
          }
          expected.remove(key);
        }
        received += 1;
        println!(
          "Consumed: topic={}, partition={}, offset={}, key={:?}, value={:?}",
          record.topic(),
          record.partition(),
          record.offset(),
          record.key(),
          record.value()
        );
      }
      consumer.commit_sync().await?;
    }
    Ok(())
  }
  .await;
  shutdown.abort();
  let close_result = consumer.close().await.map_err(Into::into);
  result.and(close_result)
}

fn env_or(name: &str, default: &str) -> String {
  std::env::var(name).unwrap_or_else(|_| default.to_owned())
}

fn positive_env(name: &str, default: usize) -> AppResult<usize> {
  let value = env_or(name, &default.to_string()).parse::<usize>()?;
  if value == 0 {
    return Err(
      io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{name} must be positive"),
      )
      .into(),
    );
  }
  Ok(value)
}

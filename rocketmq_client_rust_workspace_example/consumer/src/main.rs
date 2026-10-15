use std::{env, error::Error, io, sync::Arc};

use rocketmq_client_rust::{
  ClientResult, ClientRuntime, ClientRuntimeConfig, ConsumeConcurrentlyContext,
  ConsumeConcurrentlyStatus, DefaultMQPushConsumer, MQPushConsumer, MessageListenerConcurrently,
};
use rocketmq_model::common::message::{MessageTrait, message_ext::MessageExt};
use rocketmq_observability::{TelemetryBootstrapConfig, install_global};
use rocketmq_runtime::{RuntimeOwner, wait_for_signal_result};

type DemoResult<T> = Result<T, Box<dyn Error>>;

struct Config {
  namesrv_addr: String,
  consumer_group: String,
  topic: String,
  tags: String,
}

impl Config {
  fn from_env() -> DemoResult<Self> {
    Ok(Self {
      namesrv_addr: env_or_default("NAMESRV_ADDR", "127.0.0.1:9876")?,
      consumer_group: env_or_default("CONSUMER_GROUP", "example_consumer_group")?,
      topic: env_or_default("TOPIC", "TopicTest")?,
      tags: env_or_default("TAGS", "*")?,
    })
  }
}

fn env_or_default(name: &str, default: &str) -> DemoResult<String> {
  match env::var(name) {
    Ok(value) if value.trim().is_empty() => Err(
      io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{name} must not be empty"),
      )
      .into(),
    ),
    Ok(value) => Ok(value.trim().to_owned()),
    Err(env::VarError::NotPresent) => Ok(default.to_owned()),
    Err(error) => {
      Err(io::Error::new(io::ErrorKind::InvalidInput, format!("{name}: {error}")).into())
    }
  }
}

fn main() -> DemoResult<()> {
  let config = Config::from_env()?;
  let telemetry = install_global(&TelemetryBootstrapConfig::default())?;
  // 由 RuntimeOwner 管理运行时，不再额外创建 #[tokio::main] 运行时。
  let owner = RuntimeOwner::new()?;
  let client_runtime = ClientRuntime::try_new(
    owner.root_context().component("example-consumer"),
    ClientRuntimeConfig::default(),
    telemetry.handle(),
  )?;

  let (result, client_report) = owner.block_on(async {
    let result = consume_until_interrupt(Arc::clone(&client_runtime), &config).await;
    (result, client_runtime.shutdown().await)
  });
  // 即使启动或等待信号失败，也先清理 consumer、client、runtime，最后关闭日志。
  let runtime_report = owner.shutdown_runtime_blocking();
  let telemetry_report = telemetry.shutdown();
  if !client_report.is_healthy() {
    eprintln!("client shutdown: {}", client_report.to_json());
  }
  if let Ok(report) = &runtime_report
    && !report.is_healthy()
  {
    eprintln!("runtime shutdown: {}", report.to_json());
  }

  if !telemetry_report.is_healthy() {
    eprintln!("telemetry shutdown: {}", telemetry_report.to_json());
  }

  result?;

  let runtime_report = runtime_report?;
  if !client_report.is_healthy() || !runtime_report.is_healthy() || !telemetry_report.is_healthy() {
    return Err(io::Error::other("consumer shutdown did not complete cleanly").into());
  }
  Ok(())
}

struct Listener;

impl MessageListenerConcurrently for Listener {
  fn consume_message(
    &self,
    messages: &[&MessageExt],
    _context: &ConsumeConcurrentlyContext,
  ) -> ClientResult<ConsumeConcurrentlyStatus> {
    for message in messages {
      println!(
        "received: id={}, topic={}, tags={}, queue={}, offset={}, retries={}",
        message.msg_id(),
        message.topic(),
        message.tags().unwrap_or_default(),
        message.queue_id(),
        message.queue_offset(),
        message.reconsume_times(),
      );
      match message.get_body() {
        Some(body) => println!("body: {}", String::from_utf8_lossy(body)),
        None => println!("body: <empty>"),
      }
    }
    // 此 demo 仅展示消息。实际业务必须完成处理后才返回成功，并保证处理幂等。
    // 处理失败时返回 Ok(ConsumeConcurrentlyStatus::ReconsumeLater)，让 broker 重试。
    Ok(ConsumeConcurrentlyStatus::ConsumeSuccess)
  }
}

async fn consume_until_interrupt(
  client_runtime: Arc<ClientRuntime>,
  config: &Config,
) -> DemoResult<()> {
  let mut consumer = DefaultMQPushConsumer::builder(client_runtime)
    .consumer_group(config.consumer_group.clone())
    .name_server_addr(config.namesrv_addr.clone())
    // 默认 Clustering：同组实例分担消息；并发消费不保证消息顺序。
    .consume_thread_min(1)
    .consume_thread_max(10)
    .consume_message_batch_max_size(1)
    .build();
  let result = async {
    consumer
      .subscribe(config.topic.clone(), config.tags.clone())
      .await?;
    consumer.register_message_listener_concurrently(Listener);
    consumer.start().await?;
    println!(
      "consumer started: namesrv={}, group={}, topic={}, tags={}; press Ctrl+C to stop",
      config.namesrv_addr, config.consumer_group, config.topic, config.tags,
    );
    wait_for_signal_result().await?;
    Ok(())
  }
  .await;
  consumer.shutdown().await;
  println!("consumer stopped");
  result
}

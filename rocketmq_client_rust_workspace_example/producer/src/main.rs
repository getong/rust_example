use rocketmq_client_rust::{ClientResult, ClientRuntime, ClientRuntimeConfig, DefaultMQProducer};
use rocketmq_model::common::message::message_single::Message;
use rocketmq_observability::TelemetryRuntimeGuard;
use rocketmq_runtime::RuntimeOwner;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let owner = RuntimeOwner::new()?;
  let telemetry = TelemetryRuntimeGuard::noop();
  let client_runtime = ClientRuntime::try_new(
    owner.root_context().component("example-client"),
    ClientRuntimeConfig::default(),
    telemetry.handle(),
  )?;

  let (result, client_report) = owner.block_on(async {
    let mut producer = DefaultMQProducer::builder(client_runtime.clone())
      .producer_group("example_producer_group")
      .name_server_addr("127.0.0.1:9876")
      .build();
    let result: ClientResult<()> = async {
      producer.start().await?;
      let message = Message::builder()
        .topic("TopicTest")
        .body_slice(b"Hello RocketMQ")
        .build_unchecked();
      let result = producer.send_with_timeout(message, 2000).await?;
      println!("send result: {result:?}");
      Ok(())
    }
    .await;
    producer.shutdown().await;
    (result, client_runtime.shutdown().await)
  });
  let runtime_report = owner.shutdown_runtime_blocking()?;
  result?;
  if !client_report.is_healthy() || !runtime_report.is_healthy() {
    return Err(std::io::Error::other("client shutdown did not complete cleanly").into());
  }
  Ok(())
}

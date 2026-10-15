use queen_mq::{Config, Queen, SubscriptionMode};

#[tokio::main]
async fn main() -> queen_mq::Result<()> {
  tracing_subscriber::fmt::init();
  println!("Starting Queen MQ example: http://localhost:6632");
  let queen = Queen::connect(Config::new("http://localhost:6632"))?;

  // A queue is created on first push; configure it only when you want
  // something other than the defaults.
  println!("Publishing to orders / customer-42...");
  let results = queen
    .queue("orders")
    .partition("customer-42")
    .push(serde_json::json!({ "total": 19.99 }))
    .await?;
  println!("Push results: {results:?}");

  println!("Consuming orders as fulfilment; waiting for messages (Ctrl+C to stop)...");
  queen
    .queue("orders")
    .group("fulfilment")
    // A new group defaults to New, which skips the message pushed above.
    // All reads retained messages; existing groups keep their saved cursor.
    .subscription_mode(SubscriptionMode::All)
    .batch(10)
    .consume(|msg| async move {
      println!("{}", msg.data);
      Ok::<_, std::convert::Infallible>(())
    })
    .await?;

  Ok(())
}

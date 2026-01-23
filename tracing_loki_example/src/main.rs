use std::process;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use url::Url;

#[tokio::main]
async fn main() -> Result<(), tracing_loki::Error> {
  let (layer, task) = tracing_loki::builder()
    .label("host", "mine")?
    .extra_field("pid", format!("{}", process::id()))?
    .build_url(Url::parse("http://127.0.0.1:3100").unwrap())?;

  // We need to register our layer with `tracing`.
  tracing_subscriber::registry()
    .with(layer)
    // One could add more layers here, for example logging to stdout:
    // .with(tracing_subscriber::fmt::Layer::new())
    .init();

  // The background task needs to be spawned so the logs actually get
  // delivered.
  tokio::spawn(task);

  tracing::info!(
    task = "tracing_setup",
    result = "success",
    "tracing successfully set up",
  );

  Ok(())
}

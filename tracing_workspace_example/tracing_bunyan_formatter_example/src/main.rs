use tracing::{info, instrument};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{layer::SubscriberExt, Registry};

#[instrument]
pub fn a_unit_of_work(first_parameter: u64) {
  for i in 0 .. 2 {
    a_sub_unit_of_work(i);
  }
  info!(excited = "true", "Tracing is quite cool!");
}

#[instrument]
pub fn a_sub_unit_of_work(sub_parameter: u64) {
  info!("Events have the full context of their parent span!");
}

fn main() {
  let formatting_layer = BunyanFormattingLayer::new("tracing_demo".into(), std::io::stdout);
  let subscriber = Registry::default()
    .with(JsonStorageLayer)
    .with(formatting_layer);
  tracing::subscriber::set_global_default(subscriber).unwrap();

  info!("Orphan event without a parent span");
  a_unit_of_work(2);
}

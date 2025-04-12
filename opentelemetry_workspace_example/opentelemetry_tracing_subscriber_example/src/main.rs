use std::time::Duration;

use opentelemetry::{KeyValue, global, trace::TracerProvider};
use opentelemetry_otlp::{WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{
  Resource,
  // metrics::Temporality,
  trace::{RandomIdGenerator, Sampler},
};
use tonic::metadata::*;
use tracing::{error, span};
use tracing_subscriber::{Registry, layer::SubscriberExt};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
  let mut map = MetadataMap::with_capacity(3);

  map.insert("x-host", "example.com".parse().unwrap());
  map.insert("x-number", "123".parse().unwrap());
  map.insert_bin(
    "trace-proto-bin",
    MetadataValue::from_bytes(b"[binary data]"),
  );
  let exporter = opentelemetry_otlp::SpanExporter::builder()
    .with_tonic()
    .with_endpoint("http://localhost:4317")
    .with_timeout(Duration::from_secs(3))
    .with_metadata(map)
    .build()?;

  let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
    .with_batch_exporter(exporter)
    .with_sampler(Sampler::AlwaysOn)
    .with_id_generator(RandomIdGenerator::default())
    .with_max_events_per_span(64)
    .with_max_attributes_per_span(16)
    .with_resource(
      Resource::builder_empty()
        .with_attributes([KeyValue::new("service.name", "example")])
        .build(),
    )
    .build();
  global::set_tracer_provider(tracer_provider.clone());
  // let tracer = global::tracer("tracer-name");
  let tracer = tracer_provider.tracer("tracer-name");

  // Create a tracing layer with the configured tracer
  let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

  // Use the tracing subscriber `Registry`, or any other subscriber
  // that impls `LookupSpan`
  let subscriber = Registry::default().with(telemetry);

  // Trace executed code
  tracing::subscriber::with_default(subscriber, || {
    // Spans will be sent to the configured OpenTelemetry exporter
    let root = span!(tracing::Level::TRACE, "app_start", work_units = 2);
    let _enter = root.enter();

    error!("This event will be logged in the root span.");
  });

  Ok(())
}

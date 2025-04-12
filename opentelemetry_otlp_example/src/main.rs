use std::time::Duration;

use opentelemetry::{global, trace::Tracer, KeyValue};
use opentelemetry_otlp::{Protocol, WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{
  // metrics::Temporality,
  trace::{RandomIdGenerator, Sampler},
  Resource,
};
use tonic::metadata::*;

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
  let tracer = global::tracer("tracer-name");

  let exporter = opentelemetry_otlp::MetricExporter::builder()
    .with_tonic()
    .with_endpoint("http://localhost:4318/v1/metrics")
    .with_protocol(Protocol::Grpc)
    .with_timeout(Duration::from_secs(3))
    .build()
    .unwrap();

  let _provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
    .with_periodic_exporter(exporter)
    .with_resource(
      Resource::builder_empty()
        .with_attributes([KeyValue::new("service.name", "example")])
        .build(),
    )
    .build();

  tracer.in_span("doing_work", |_cx| {
    // Traced app logic here...
  });

  Ok(())
}

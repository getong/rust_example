use std::time::Duration;

use opentelemetry::{trace::Tracer, KeyValue};
use opentelemetry_otlp::{ExportConfig, Protocol, WithExportConfig};
use opentelemetry_sdk::{
  metrics::reader::{DefaultAggregationSelector, DefaultTemporalitySelector},
  trace::{self, RandomIdGenerator, Sampler},
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

  let tracer = opentelemetry_otlp::new_pipeline()
    .tracing()
    .with_exporter(
      opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint("http://localhost:4317")
        .with_timeout(Duration::from_secs(3))
        .with_metadata(map),
    )
    .with_trace_config(
      trace::config()
        .with_sampler(Sampler::AlwaysOn)
        .with_id_generator(RandomIdGenerator::default())
        .with_max_events_per_span(64)
        .with_max_attributes_per_span(16)
        .with_max_events_per_span(16)
        .with_resource(Resource::new(vec![KeyValue::new(
          "service.name",
          "example",
        )])),
    )
    .install_batch(opentelemetry_sdk::runtime::Tokio)?;

  let export_config = ExportConfig {
    endpoint: "http://localhost:4317".to_string(),
    timeout: Duration::from_secs(3),
    protocol: Protocol::Grpc,
  };

  let _meter = opentelemetry_otlp::new_pipeline()
    .metrics(opentelemetry_sdk::runtime::Tokio)
    .with_exporter(
      opentelemetry_otlp::new_exporter()
        .tonic()
        .with_export_config(export_config),
      // can also config it using with_* functions like the tracing part above.
    )
    .with_resource(Resource::new(vec![KeyValue::new(
      "service.name",
      "example",
    )]))
    .with_period(Duration::from_secs(3))
    .with_timeout(Duration::from_secs(10))
    .with_aggregation_selector(DefaultAggregationSelector::new())
    .with_temporality_selector(DefaultTemporalitySelector::new())
    .build();

  tracer.in_span("doing_work", |_cx| {
    // Traced app logic here...
  });

  Ok(())
}

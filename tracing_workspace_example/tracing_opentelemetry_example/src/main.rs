// copy from https://medium.com/@rasnaut/the-easiest-way-to-send-traces-from-the-rust-app-to-grafana-cloud-7a66baf2e45b
use std::{error::Error, thread, time::Duration};

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{self, WithExportConfig};
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::{event, info_span, span, warn, Level};
// use tracing_attributes::instrument;
use tracing_opentelemetry;
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
  let otlp_exporter = opentelemetry_otlp::SpanExporter::builder()
    .with_tonic()
    .with_endpoint("http://0.0.0.0:4317")
    .build()?;

  let provider = SdkTracerProvider::builder()
    .with_batch_exporter(otlp_exporter)
    .build();

  let tracer = provider.tracer("tracing_opentelemetry_example");

  let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);

  tracing_subscriber::registry()
    .with(opentelemetry)
    .try_init()?;

  {
    let root = span!(Level::INFO, "app_start", "work_units" = "2");
    let _enter = root.enter();

    span!(Level::INFO, "faster_work").in_scope(|| thread::sleep(Duration::from_millis(10)));

    info_span!("real_work2").in_scope(|| thread::sleep(Duration::from_millis(10)));

    event!(Level::TRACE, "Just Trace");
    warn!("About to exit!");
  }

  provider.shutdown()?;
  Ok(())
}

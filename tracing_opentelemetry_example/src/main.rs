// copy from https://medium.com/@rasnaut/the-easiest-way-to-send-traces-from-the-rust-app-to-grafana-cloud-7a66baf2e45b
use std::{error::Error, thread, time::Duration};

use opentelemetry;
use opentelemetry::global::shutdown_tracer_provider;
use opentelemetry_otlp::{self, WithExportConfig};
use tracing::{event, info_span, span, warn, Level};
// use tracing_attributes::instrument;
use tracing_opentelemetry;
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    let otlp_exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint("http://0.0.0.0:4317");

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(otlp_exporter)
        .install_batch(opentelemetry::runtime::AsyncStd)
        .expect("failed to install");

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

    shutdown_tracer_provider();
    Ok(())
}

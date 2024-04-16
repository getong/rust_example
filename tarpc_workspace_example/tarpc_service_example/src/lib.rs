// Copyright 2018 Google LLC
//
// Use of this source code is governed by an MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT.

use opentelemetry_otlp::WithExportConfig;
use std::env;
use tracing_subscriber::{fmt::format::FmtSpan, prelude::*};
/// This is the service definition. It looks a lot like a trait definition.
/// It defines one RPC, hello, which takes one arg, name, and returns a String.
#[tarpc::service]
pub trait World {
  /// Returns a greeting for name.
  async fn hello(name: String) -> String;
}

/// Initializes an OpenTelemetry tracing subscriber with a Jaeger backend.
pub fn init_tracing() -> anyhow::Result<()> {
  env::set_var("OTEL_BSP_MAX_EXPORT_BATCH_SIZE", "12");

  let otlp_exporter = opentelemetry_otlp::new_exporter()
    .tonic()
    .with_endpoint("http://0.0.0.0:4317");
  let tracer = opentelemetry_otlp::new_pipeline()
    .tracing()
    .with_exporter(otlp_exporter)
    .install_batch(opentelemetry_sdk::runtime::Tokio)
    .expect("failed to install");

  tracing_subscriber::registry()
    .with(tracing_subscriber::EnvFilter::from_default_env())
    .with(tracing_subscriber::fmt::layer().with_span_events(FmtSpan::NEW | FmtSpan::CLOSE))
    .with(tracing_opentelemetry::layer().with_tracer(tracer))
    .try_init()?;

  Ok(())
}

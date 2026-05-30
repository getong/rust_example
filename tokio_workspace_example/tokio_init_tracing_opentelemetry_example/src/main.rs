// use axum_tracing_opentelemetry::opentelemetry_tracing_layer;

#[tokio::main]
async fn main() -> Result<(), axum::BoxError> {
  // very opinionated init of tracing, look as is source to compose your own
  let _guard = init_tracing_opentelemetry::TracingConfig::production().init_subscriber()?;

  // ...;

  Ok(())
}

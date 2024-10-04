use axum_tracing_opentelemetry::opentelemetry_tracing_layer;

#[tokio::main]
async fn main() -> Result<(), axum::BoxError> {
  // very opinionated init of tracing, look as is source to compose your own
  init_tracing_opentelemetry::tracing_subscriber_ext::init_subscribers()?;

  // ...;

  Ok(())
}

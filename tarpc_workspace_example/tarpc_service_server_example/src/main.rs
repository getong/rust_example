use clap::Parser;
use futures::{future, prelude::*};
use opentelemetry::trace::TracerProvider as _;
use rand::{
  distributions::{Distribution, Uniform},
  thread_rng,
};
use std::{
  net::{IpAddr, Ipv6Addr, SocketAddr},
  time::Duration,
};
use tarpc::{
  context,
  server::{self, incoming::Incoming, Channel},
  tokio_serde::formats::Json,
};
use tokio::time;
use tracing_subscriber::{fmt::format::FmtSpan, prelude::*};

/// This is the service definition. It looks a lot like a trait definition.
/// It defines one RPC, hello, which takes one arg, name, and returns a String.
#[tarpc::service]
pub trait World {
  /// Returns a greeting for name.
  async fn hello(name: String) -> String;
}

/// Initializes an OpenTelemetry tracing subscriber with a OTLP backend.
pub fn init_tracing(service_name: &'static str) -> anyhow::Result<()> {
  let tracer_provider = opentelemetry_otlp::new_pipeline()
    .tracing()
    .with_trace_config(opentelemetry_sdk::trace::Config::default().with_resource(
      opentelemetry_sdk::Resource::new([opentelemetry::KeyValue::new(
        opentelemetry_semantic_conventions::resource::SERVICE_NAME,
        service_name,
      )]),
    ))
    .with_batch_config(opentelemetry_sdk::trace::BatchConfig::default())
    .with_exporter(opentelemetry_otlp::new_exporter().tonic())
    .install_batch(opentelemetry_sdk::runtime::Tokio)?;
  opentelemetry::global::set_tracer_provider(tracer_provider.clone());
  let tracer = tracer_provider.tracer(service_name);

  tracing_subscriber::registry()
    .with(tracing_subscriber::EnvFilter::from_default_env())
    .with(tracing_subscriber::fmt::layer().with_span_events(FmtSpan::NEW | FmtSpan::CLOSE))
    .with(tracing_opentelemetry::layer().with_tracer(tracer))
    .try_init()?;

  Ok(())
}

#[derive(Parser)]
struct Flags {
  /// Sets the port number to listen on.
  #[clap(long)]
  port: u16,
}

// This is the type that implements the generated World trait. It is the business logic
// and is used to start the server.
#[derive(Clone)]
struct HelloServer(SocketAddr);

impl World for HelloServer {
  async fn hello(self, _: context::Context, name: String) -> String {
    let sleep_time = Duration::from_millis(Uniform::new_inclusive(1, 10).sample(&mut thread_rng()));
    time::sleep(sleep_time).await;
    format!("Hello, {name}! You are connected from {}", self.0)
  }
}

async fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
  tokio::spawn(fut);
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let flags = Flags::parse();
  init_tracing("Tarpc Example Server")?;

  let server_addr = (IpAddr::V6(Ipv6Addr::LOCALHOST), flags.port);

  // JSON transport is provided by the json_transport tarpc module. It makes it easy
  // to start up a serde-powered json serialization strategy over TCP.
  let mut listener = tarpc::serde_transport::tcp::listen(&server_addr, Json::default).await?;
  tracing::info!("Listening on port {}", listener.local_addr().port());
  listener.config_mut().max_frame_length(usize::MAX);
  listener
    // Ignore accept errors.
    .filter_map(|r| future::ready(r.ok()))
    .map(server::BaseChannel::with_defaults)
    // Limit channels to 1 per IP.
    .max_channels_per_key(1, |t| t.transport().peer_addr().unwrap().ip())
    // serve is generated by the service attribute. It takes as input any type implementing
    // the generated World trait.
    .map(|channel| {
      let server = HelloServer(channel.transport().peer_addr().unwrap());
      channel.execute(server.serve()).for_each(spawn)
    })
    // Max 10 channels.
    .buffer_unordered(10)
    .for_each(|_| async {})
    .await;

  Ok(())
}

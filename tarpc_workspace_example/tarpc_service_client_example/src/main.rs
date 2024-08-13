use clap::Parser;
use opentelemetry::trace::TracerProvider as _;
use std::{net::SocketAddr, time::Duration};
use tarpc::{client, context, tokio_serde::formats::Json};
use tokio::time::sleep;
use tracing::Instrument;
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
  /// Sets the server address to connect to.
  #[clap(long)]
  server_addr: SocketAddr,
  /// Sets the name to say hello to.
  #[clap(long)]
  name: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let flags = Flags::parse();
  init_tracing("Tarpc Example Client")?;

  let mut transport = tarpc::serde_transport::tcp::connect(flags.server_addr, Json::default);
  transport.config_mut().max_frame_length(usize::MAX);

  // WorldClient is generated by the service attribute. It has a constructor `new` that takes a
  // config and any Transport as input.
  let client = WorldClient::new(client::Config::default(), transport.await?).spawn();

  let hello = async move {
    // Send the request twice, just to be safe! ;)
    tokio::select! {
      hello1 = client.hello(context::current(), format!("{}1", flags.name)) => { hello1 }
      hello2 = client.hello(context::current(), format!("{}2", flags.name)) => { hello2 }
    }
  }
  .instrument(tracing::info_span!("Two Hellos"))
  .await;

  match hello {
    Ok(hello) => {
      println!("{:?}", hello);
      tracing::info!("{hello:?}");
    }
    Err(e) => tracing::warn!("{:?}", anyhow::Error::from(e)),
  }

  // Let the background span processor finish.
  sleep(Duration::from_micros(1)).await;
  opentelemetry::global::shutdown_tracer_provider();

  Ok(())
}

use anyhow::Result;
use clap::Parser;
use iroh_relay_example::{
  cli::{Cli, Command},
  client, demo, relay, server,
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
    .with_writer(std::io::stderr)
    .init();
  match Cli::parse().command {
    Command::Relay { bind, metrics } => relay::run(bind, metrics).await,
    Command::Server(network) => server::run(&network).await,
    Command::Client {
      network,
      server,
      message,
    } => {
      client::run(&network, server, message.as_bytes()).await?;
      Ok(())
    }
    Command::Demo { message } => demo::run(message.as_bytes()).await,
  }
}

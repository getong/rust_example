mod cli;

use clap::Parser;
use cli::{Cli, Command};
use iroh_mainline_kad::{
  run_blob_get, run_blob_seed, run_client, run_gossip, run_kad_server, run_local_demo, run_server,
};
use n0_error::Result;
use tracing_subscriber::{EnvFilter, prelude::*};

#[tokio::main]
async fn main() -> Result<()> {
  setup_logging();
  let cli = Cli::parse();

  match cli.command {
    Command::Server(args) => run_server(args.into_options()?).await,
    Command::Client(args) => run_client(args.into_options()?).await,
    Command::KadServer(args) => run_kad_server(args.into_options()).await,
    Command::Gossip(args) => run_gossip(args.into_options()?).await,
    Command::BlobSeed(args) => run_blob_seed(args.into_options()?).await,
    Command::BlobGet(args) => run_blob_get(args.into_options()?).await,
    Command::LocalDemo(args) => run_local_demo(args.into_options()).await,
  }
}

fn setup_logging() {
  tracing_subscriber::registry()
    .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
    .with(EnvFilter::from_default_env())
    .try_init()
    .ok();
}

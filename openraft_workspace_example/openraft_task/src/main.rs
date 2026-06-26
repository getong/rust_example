use clap::Parser;
use openraft_task::{Opt, run};
use tracing_subscriber::{EnvFilter, prelude::*};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  init_tracing();
  run(Opt::parse()).await
}

fn init_tracing() {
  tracing_subscriber::registry()
    .with(
      tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true)
        .with_filter(EnvFilter::from_default_env()),
    )
    .init();
}

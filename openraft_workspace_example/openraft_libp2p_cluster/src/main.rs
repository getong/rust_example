use clap::Parser;
use openraft_libp2p_cluster::{
  app::{Opt, run},
  telemetry,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let opt = Opt::parse();
  telemetry::init_tracing(!opt.no_tokio_console);

  run(opt).await
}

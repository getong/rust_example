use clap::Parser;
use libp2p_openraft_multi_raft_rocksdb::app::{Opt, run};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  tracing_subscriber::fmt()
    .with_target(true)
    .with_thread_ids(true)
    .with_level(true)
    .with_ansi(false)
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  let opt = Opt::parse();
  run(opt).await
}

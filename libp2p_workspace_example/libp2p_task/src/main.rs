mod app;
mod domain;
mod journal;
mod kv_types;
mod network;
mod openraft_groups;
mod raft_role;
mod raft_store;
mod state;
mod worker;

use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init()
    .ok();

  let opt = app::Opt::parse();
  app::run(opt).await
}

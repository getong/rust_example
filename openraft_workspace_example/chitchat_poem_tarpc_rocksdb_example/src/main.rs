use chitchat_poem_tarpc_rocksdb_example::{
  common::Opt, stract_integration::start_distributed_raft_node,
};
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  // Setup the logger
  tracing_subscriber::fmt()
    .with_target(true)
    .with_thread_ids(true)
    .with_level(true)
    .with_ansi(false)
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  // Parse the parameters passed by arguments.
  let options = Opt::parse();

  // Use Stract-inspired distributed raft node startup
  start_distributed_raft_node(options.id, format!("{}.db", options.rpc_addr), options).await
}

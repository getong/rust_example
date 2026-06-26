use clap::Parser;
use openraft_kameo::start_kameo_raft_node;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
struct Opt {
  #[arg(long)]
  id: u64,

  #[arg(long)]
  http_addr: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  tracing_subscriber::fmt()
    .with_target(true)
    .with_thread_ids(true)
    .with_level(true)
    .with_ansi(false)
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  let options = Opt::parse();
  start_kameo_raft_node(options.id, options.http_addr).await
}

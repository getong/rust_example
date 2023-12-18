use chitchat::transport::UdpTransport;
use chitchat::{spawn_chitchat, Chitchat, ChitchatConfig, ChitchatId, FailureDetectorConfig};
use once_cell::sync::OnceCell;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use clap::Parser;

use cool_id_generator::Size;

static SHARED_DATA: OnceCell<Arc<Mutex<Chitchat>>> = OnceCell::new();

fn generate_server_id(public_addr: SocketAddr) -> String {
  let cool_id = cool_id_generator::get_id(Size::Medium);
  format!("server:{public_addr}-{cool_id}")
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Opt {
  /// Defines the socket addr on which we should listen to.
  #[arg(long = "listen_addr", default_value = "127.0.0.1:10000")]
  listen_addr: SocketAddr,
  /// Defines the socket address (host:port) other servers should use to
  /// reach this server.
  ///
  /// It defaults to the listen address, but this is only valid
  /// when all server are running on the same server.
  #[arg(long = "public_addr")]
  public_addr: Option<SocketAddr>,

  /// Node ID. Must be unique. If None, the node ID will be generated from
  /// the public_addr and a random suffix.
  #[arg(long)]
  node_id: Option<String>,

  #[arg(long = "seed")]
  seeds: Vec<String>,

  #[arg(long, default_value_t = 500)]
  interval: u64,

  #[arg(long = "subscriber_port", default_value_t = 5000)]
  subscriber_port: u16,
}

// # First server
// cargo run -- --listen_addr 127.0.0.1:10000 --subscriber_port 5000

// # Second server
// cargo run -- --listen_addr 127.0.0.1:10001 --seed localhost:10000 --subscriber_port 5001

// tokio-console http://127.0.0.1:5000
// tokio-console http://127.0.0.1:5001

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let opt = Opt::parse();
  console_subscriber::ConsoleLayer::builder()
    // set the address the server is bound to
    .server_addr(([127, 0, 0, 1], opt.subscriber_port))
    // ... other configurations ...
    .init();
  let public_addr = opt.public_addr.unwrap_or(opt.listen_addr);
  let node_id = opt
    .node_id
    .unwrap_or_else(|| generate_server_id(public_addr));

  let chitchat_id = ChitchatId::new(node_id, 0, public_addr);
  let config = ChitchatConfig {
    cluster_id: "testing".to_string(),
    chitchat_id,
    gossip_interval: Duration::from_millis(opt.interval),
    listen_addr: opt.listen_addr,
    seed_nodes: opt.seeds.clone(),
    failure_detector_config: FailureDetectorConfig::default(),
    marked_for_deletion_grace_period: 10_000,
  };

  let chitchat_handler = spawn_chitchat(config, Vec::new(), &UdpTransport).await?;
  let chitchat = chitchat_handler.chitchat();

  SHARED_DATA.get_or_init(|| chitchat);
  loop {}

  // Ok(())
}

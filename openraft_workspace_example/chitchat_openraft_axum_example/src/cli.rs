use std::net::SocketAddr;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "chitchat", about = "Chitchat cluster example with services.")]
pub struct Opt {
  /// Defines the socket addr on which we should listen to.
  #[arg(long = "listen_addr", default_value = "127.0.0.1:10000")]
  pub listen_addr: SocketAddr,

  /// Defines the gossip address for chitchat
  #[arg(long = "gossip_addr")]
  pub gossip_addr: Option<SocketAddr>,

  /// Node ID. Must be unique. If None, the node ID will be generated.
  #[arg(long = "node_id")]
  pub node_id: Option<String>,

  /// Seed nodes for joining the cluster
  #[arg(long = "seed")]
  pub seeds: Vec<String>,

  /// Gossip interval in milliseconds
  #[arg(long = "interval_ms", default_value = "1000")]
  pub interval: u64,

  /// Service type to run on this node
  #[arg(long = "service", default_value = "api_gateway")]
  pub service_type: String,

  /// Shard ID for services that require sharding
  #[arg(long = "shard")]
  pub shard: Option<u64>,

  /// Run demo with 5 predefined nodes
  #[arg(long = "demo")]
  pub demo: bool,

  /// Automatically find available ports if the specified port is in use
  #[arg(long = "auto_port")]
  pub auto_port: bool,
}

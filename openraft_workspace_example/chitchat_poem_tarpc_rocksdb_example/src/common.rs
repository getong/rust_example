use std::{collections::BTreeMap, net::SocketAddr, sync::Arc};

use chitchat::Chitchat;
use clap::Parser;
use openraft::Config;
use tokio::sync::{Mutex, RwLock};

use crate::{ExampleRaft, NodeId};

#[derive(Clone)]
pub struct Api {
  // pub num: Arc<Mutex<i64>>,
  pub id: NodeId,
  pub api_addr: String,
  pub rpc_addr: String,
  pub raft: ExampleRaft,
  pub key_values: Arc<RwLock<BTreeMap<String, String>>>,
  pub config: Arc<Config>,
}

#[derive(Parser, Clone, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Opt {
  #[clap(long)]
  pub id: u64,

  /// API server address (HTTP)
  #[clap(long, default_value = "127.0.0.1:8080")]
  pub api_addr: String,

  /// RPC address for raft communication
  #[clap(long)]
  pub rpc_addr: String,

  /// Legacy HTTP address (deprecated, use api_addr)
  #[clap(long)]
  pub http_addr: Option<String>,

  /// Chitchat gossip address for service discovery
  #[clap(long, default_value = "127.0.0.1:9000")]
  pub gossip_addr: String,

  /// Seed gossip addresses for joining existing cluster
  #[clap(long)]
  pub seed_gossip_addrs: Vec<String>,

  /// Defines the socket addr on which we should listen to.
  #[arg(long = "listen_addr", default_value = "127.0.0.1:10000")]
  pub listen_addr: SocketAddr,

  /// Defines the socket address (host:port) other servers should use to
  /// reach this server.
  ///
  /// It defaults to the listen address, but this is only valid
  /// when all server are running on the same server.
  #[arg(long = "public_addr")]
  pub public_addr: Option<SocketAddr>,

  /// Node ID. Must be unique. If None, the node ID will be generated from
  /// the public_addr and a random suffix.
  #[arg(long = "node_id")]
  pub node_id: Option<String>,

  #[arg(long = "seed")]
  pub seeds: Vec<String>,

  #[arg(long, default_value_t = 500)]
  pub interval: u64,
}

pub struct ChitchatApi {
  pub chitchat: Arc<Mutex<Chitchat>>,
}

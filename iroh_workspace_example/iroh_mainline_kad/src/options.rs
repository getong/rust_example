use std::{
  net::{Ipv4Addr, SocketAddr},
  path::PathBuf,
  time::Duration,
};

use iroh_blobs::Hash;
use iroh_gossip::TopicId;

use crate::identity::ClusterIdentity;

#[derive(Debug, Clone)]
pub struct DhtOptions {
  pub server_mode: bool,
  pub bind: Ipv4Addr,
  pub port: Option<u16>,
  pub bootstrap: Vec<String>,
  pub request_timeout: Duration,
}

impl Default for DhtOptions {
  fn default() -> Self {
    Self {
      server_mode: false,
      bind: Ipv4Addr::UNSPECIFIED,
      port: None,
      bootstrap: Vec::new(),
      request_timeout: Duration::from_secs(4),
    }
  }
}

#[derive(Debug, Clone)]
pub struct IrohOptions {
  pub bind: SocketAddr,
  pub relay: bool,
  pub wait_online: Duration,
}

impl Default for IrohOptions {
  fn default() -> Self {
    Self {
      bind: SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
      relay: true,
      wait_online: Duration::from_secs(15),
    }
  }
}

#[derive(Debug, Clone)]
pub struct ServerOptions {
  pub cluster: ClusterIdentity,
  pub dht: DhtOptions,
  pub iroh: IrohOptions,
  pub name: String,
  pub republish_every: Duration,
}

#[derive(Debug, Clone)]
pub struct ClientOptions {
  pub cluster: ClusterIdentity,
  pub dht: DhtOptions,
  pub iroh: IrohOptions,
  pub message: String,
  pub discover_timeout: Duration,
  pub connect_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct GossipOptions {
  pub cluster: ClusterIdentity,
  pub dht: DhtOptions,
  pub iroh: IrohOptions,
  pub name: String,
  pub topic: TopicId,
  pub message: Option<String>,
  pub discover_timeout: Duration,
  pub wait_joined: Duration,
  pub republish_every: Duration,
  pub exit_after_broadcast: bool,
}

#[derive(Debug, Clone)]
pub struct BlobSeedOptions {
  pub cluster: ClusterIdentity,
  pub dht: DhtOptions,
  pub iroh: IrohOptions,
  pub name: String,
  pub file: PathBuf,
  pub store_path: PathBuf,
  pub republish_every: Duration,
}

#[derive(Debug, Clone)]
pub struct BlobGetOptions {
  pub cluster: ClusterIdentity,
  pub dht: DhtOptions,
  pub iroh: IrohOptions,
  pub hash: Hash,
  pub output: PathBuf,
  pub store_path: PathBuf,
  pub discover_timeout: Duration,
  pub request_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct LocalDemoOptions {
  pub dht_nodes: usize,
  pub servers: usize,
  pub message: String,
  pub discover_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct KadServerOptions {
  pub nodes: usize,
  pub bind: Ipv4Addr,
}

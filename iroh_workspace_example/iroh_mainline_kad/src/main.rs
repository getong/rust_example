use std::net::{Ipv4Addr, SocketAddr};

use clap::{Parser, Subcommand};
use iroh_mainline_kad::{
  BlobGetOptions, BlobSeedOptions, ClientOptions, ClusterIdentity, DEFAULT_GOSSIP_TOPIC_HEX,
  DhtOptions, GossipOptions, IrohOptions, KadServerOptions, LocalDemoOptions, ServerOptions,
  default_cluster_salt, parse_blob_hash, parse_bootstrap, parse_dht_port, parse_duration_secs,
  parse_gossip_topic, run_blob_get, run_blob_seed, run_client, run_gossip, run_kad_server,
  run_local_demo, run_server,
};
use n0_error::Result;
use tracing_subscriber::{EnvFilter, prelude::*};

#[derive(Debug, Parser)]
#[command(version, about = "Iroh cluster discovery over Mainline Kademlia DHT")]
struct Cli {
  #[command(subcommand)]
  command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
  /// Run an iroh server and publish its endpoint address into Mainline DHT.
  Server(ServerArgs),
  /// Discover a published iroh server through Mainline DHT and send one request.
  Client(ClientArgs),
  /// Run a local Mainline KAD bootstrap network for multi-process examples.
  KadServer(KadServerArgs),
  /// Run an iroh-gossip pubsub peer discovered through Mainline KAD.
  Gossip(GossipArgs),
  /// Seed a local file over iroh-blobs and publish provider info into Mainline KAD.
  BlobSeed(BlobSeedArgs),
  /// Download a blob from discovered iroh-blobs providers and export it to a file.
  BlobGet(BlobGetArgs),
  /// Run a local Mainline testnet plus iroh servers and client in one process.
  LocalDemo(LocalDemoArgs),
}

#[derive(Debug, Parser)]
struct ServerArgs {
  #[arg(long, default_value = "iroh-mainline-node")]
  name: String,
  #[arg(long)]
  cluster_secret: Option<String>,
  #[arg(long)]
  cluster_salt: Option<String>,
  #[arg(long, default_value_t = Ipv4Addr::UNSPECIFIED)]
  dht_bind: Ipv4Addr,
  #[arg(long, default_value_t = 0)]
  dht_port: u16,
  #[arg(long, value_delimiter = ',')]
  bootstrap: Vec<String>,
  #[arg(long, default_value_t = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))]
  iroh_bind: SocketAddr,
  #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
  relay: bool,
  #[arg(long, default_value_t = 15)]
  wait_online_secs: u64,
  #[arg(long, default_value_t = 4)]
  request_timeout_secs: u64,
  #[arg(long, default_value_t = 300)]
  republish_secs: u64,
}

#[derive(Debug, Parser)]
struct ClientArgs {
  #[arg(long)]
  cluster_secret: Option<String>,
  #[arg(long)]
  cluster_salt: Option<String>,
  #[arg(long, default_value_t = Ipv4Addr::UNSPECIFIED)]
  dht_bind: Ipv4Addr,
  #[arg(long, default_value_t = 0)]
  dht_port: u16,
  #[arg(long, value_delimiter = ',')]
  bootstrap: Vec<String>,
  #[arg(long, default_value_t = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))]
  iroh_bind: SocketAddr,
  #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
  relay: bool,
  #[arg(long, default_value_t = 15)]
  wait_online_secs: u64,
  #[arg(long, default_value = "hello from mainline kad client")]
  message: String,
  #[arg(long, default_value_t = 20)]
  discover_timeout_secs: u64,
  #[arg(long, default_value_t = 10)]
  connect_timeout_secs: u64,
  #[arg(long, default_value_t = 4)]
  request_timeout_secs: u64,
}

#[derive(Debug, Parser)]
struct LocalDemoArgs {
  #[arg(long, default_value_t = 5)]
  dht_nodes: usize,
  #[arg(long, default_value_t = 2)]
  servers: usize,
  #[arg(long, default_value = "hello from local demo")]
  message: String,
  #[arg(long, default_value_t = 10)]
  discover_timeout_secs: u64,
}

#[derive(Debug, Parser)]
struct KadServerArgs {
  #[arg(long, default_value_t = 5)]
  nodes: usize,
  #[arg(long, default_value_t = Ipv4Addr::LOCALHOST)]
  bind: Ipv4Addr,
}

#[derive(Debug, Parser)]
struct GossipArgs {
  #[arg(long, default_value = "iroh-gossip-node")]
  name: String,
  #[arg(long)]
  cluster_secret: Option<String>,
  #[arg(long)]
  cluster_salt: Option<String>,
  #[arg(long, default_value_t = Ipv4Addr::UNSPECIFIED)]
  dht_bind: Ipv4Addr,
  #[arg(long, default_value_t = 0)]
  dht_port: u16,
  #[arg(long, value_delimiter = ',')]
  bootstrap: Vec<String>,
  #[arg(long, default_value_t = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))]
  iroh_bind: SocketAddr,
  #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
  relay: bool,
  #[arg(long, default_value = DEFAULT_GOSSIP_TOPIC_HEX)]
  topic: String,
  #[arg(long)]
  message: Option<String>,
  #[arg(long, default_value_t = 15)]
  wait_online_secs: u64,
  #[arg(long, default_value_t = 20)]
  discover_timeout_secs: u64,
  #[arg(long, default_value_t = 10)]
  wait_joined_secs: u64,
  #[arg(long, default_value_t = 4)]
  request_timeout_secs: u64,
  #[arg(long, default_value_t = 300)]
  republish_secs: u64,
  #[arg(long, default_value_t = false, action = clap::ArgAction::SetTrue)]
  exit_after_broadcast: bool,
}

#[derive(Debug, Parser)]
struct BlobSeedArgs {
  #[arg(long, default_value = "iroh-blob-seed")]
  name: String,
  #[arg(long)]
  cluster_secret: Option<String>,
  #[arg(long)]
  cluster_salt: Option<String>,
  #[arg(long, default_value_t = Ipv4Addr::UNSPECIFIED)]
  dht_bind: Ipv4Addr,
  #[arg(long, default_value_t = 0)]
  dht_port: u16,
  #[arg(long, value_delimiter = ',')]
  bootstrap: Vec<String>,
  #[arg(long, default_value_t = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))]
  iroh_bind: SocketAddr,
  #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
  relay: bool,
  #[arg(long)]
  file: std::path::PathBuf,
  #[arg(long, default_value = ".iroh-blobs-seed")]
  store_path: std::path::PathBuf,
  #[arg(long, default_value_t = 15)]
  wait_online_secs: u64,
  #[arg(long, default_value_t = 4)]
  request_timeout_secs: u64,
  #[arg(long, default_value_t = 300)]
  republish_secs: u64,
}

#[derive(Debug, Parser)]
struct BlobGetArgs {
  #[arg(long)]
  cluster_secret: Option<String>,
  #[arg(long)]
  cluster_salt: Option<String>,
  #[arg(long, default_value_t = Ipv4Addr::UNSPECIFIED)]
  dht_bind: Ipv4Addr,
  #[arg(long, default_value_t = 0)]
  dht_port: u16,
  #[arg(long, value_delimiter = ',')]
  bootstrap: Vec<String>,
  #[arg(long, default_value_t = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))]
  iroh_bind: SocketAddr,
  #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
  relay: bool,
  #[arg(long)]
  hash: String,
  #[arg(long)]
  output: std::path::PathBuf,
  #[arg(long, default_value = ".iroh-blobs-get")]
  store_path: std::path::PathBuf,
  #[arg(long, default_value_t = 15)]
  wait_online_secs: u64,
  #[arg(long, default_value_t = 20)]
  discover_timeout_secs: u64,
  #[arg(long, default_value_t = 60)]
  request_timeout_secs: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
  setup_logging();
  let cli = Cli::parse();

  match cli.command {
    Command::Server(args) => run_server(args.into_options()?).await,
    Command::Client(args) => run_client(args.into_options()?).await,
    Command::KadServer(args) => run_kad_server(args.into_options()).await,
    Command::Gossip(args) => run_gossip(args.into_options()?).await,
    Command::BlobSeed(args) => run_blob_seed(args.into_options()?).await,
    Command::BlobGet(args) => run_blob_get(args.into_options()?).await,
    Command::LocalDemo(args) => run_local_demo(args.into_options()).await,
  }
}

impl ServerArgs {
  fn into_options(self) -> Result<ServerOptions> {
    Ok(ServerOptions {
      cluster: cluster_identity(self.cluster_secret.as_deref(), self.cluster_salt)?,
      dht: DhtOptions {
        server_mode: true,
        bind: self.dht_bind,
        port: parse_dht_port(self.dht_port),
        bootstrap: parse_bootstrap(&self.bootstrap),
        request_timeout: parse_duration_secs(self.request_timeout_secs),
      },
      iroh: IrohOptions {
        bind: self.iroh_bind,
        relay: self.relay,
        wait_online: parse_duration_secs(self.wait_online_secs),
      },
      name: self.name,
      republish_every: parse_duration_secs(self.republish_secs),
    })
  }
}

impl ClientArgs {
  fn into_options(self) -> Result<ClientOptions> {
    Ok(ClientOptions {
      cluster: cluster_identity(self.cluster_secret.as_deref(), self.cluster_salt)?,
      dht: DhtOptions {
        server_mode: false,
        bind: self.dht_bind,
        port: parse_dht_port(self.dht_port),
        bootstrap: parse_bootstrap(&self.bootstrap),
        request_timeout: parse_duration_secs(self.request_timeout_secs),
      },
      iroh: IrohOptions {
        bind: self.iroh_bind,
        relay: self.relay,
        wait_online: parse_duration_secs(self.wait_online_secs),
      },
      message: self.message,
      discover_timeout: parse_duration_secs(self.discover_timeout_secs),
      connect_timeout: parse_duration_secs(self.connect_timeout_secs),
    })
  }
}

impl LocalDemoArgs {
  fn into_options(self) -> LocalDemoOptions {
    LocalDemoOptions {
      dht_nodes: self.dht_nodes,
      servers: self.servers,
      message: self.message,
      discover_timeout: parse_duration_secs(self.discover_timeout_secs),
    }
  }
}

impl KadServerArgs {
  fn into_options(self) -> KadServerOptions {
    KadServerOptions {
      nodes: self.nodes,
      bind: self.bind,
    }
  }
}

impl GossipArgs {
  fn into_options(self) -> Result<GossipOptions> {
    Ok(GossipOptions {
      cluster: cluster_identity(self.cluster_secret.as_deref(), self.cluster_salt)?,
      dht: DhtOptions {
        server_mode: false,
        bind: self.dht_bind,
        port: parse_dht_port(self.dht_port),
        bootstrap: parse_bootstrap(&self.bootstrap),
        request_timeout: parse_duration_secs(self.request_timeout_secs),
      },
      iroh: IrohOptions {
        bind: self.iroh_bind,
        relay: self.relay,
        wait_online: parse_duration_secs(self.wait_online_secs),
      },
      name: self.name,
      topic: parse_gossip_topic(&self.topic)?,
      message: self.message,
      discover_timeout: parse_duration_secs(self.discover_timeout_secs),
      wait_joined: parse_duration_secs(self.wait_joined_secs),
      republish_every: parse_duration_secs(self.republish_secs),
      exit_after_broadcast: self.exit_after_broadcast,
    })
  }
}

impl BlobSeedArgs {
  fn into_options(self) -> Result<BlobSeedOptions> {
    Ok(BlobSeedOptions {
      cluster: cluster_identity(self.cluster_secret.as_deref(), self.cluster_salt)?,
      dht: DhtOptions {
        server_mode: false,
        bind: self.dht_bind,
        port: parse_dht_port(self.dht_port),
        bootstrap: parse_bootstrap(&self.bootstrap),
        request_timeout: parse_duration_secs(self.request_timeout_secs),
      },
      iroh: IrohOptions {
        bind: self.iroh_bind,
        relay: self.relay,
        wait_online: parse_duration_secs(self.wait_online_secs),
      },
      name: self.name,
      file: self.file,
      store_path: self.store_path,
      republish_every: parse_duration_secs(self.republish_secs),
    })
  }
}

impl BlobGetArgs {
  fn into_options(self) -> Result<BlobGetOptions> {
    Ok(BlobGetOptions {
      cluster: cluster_identity(self.cluster_secret.as_deref(), self.cluster_salt)?,
      dht: DhtOptions {
        server_mode: false,
        bind: self.dht_bind,
        port: parse_dht_port(self.dht_port),
        bootstrap: parse_bootstrap(&self.bootstrap),
        request_timeout: parse_duration_secs(self.request_timeout_secs),
      },
      iroh: IrohOptions {
        bind: self.iroh_bind,
        relay: self.relay,
        wait_online: parse_duration_secs(self.wait_online_secs),
      },
      hash: parse_blob_hash(&self.hash)?,
      output: self.output,
      store_path: self.store_path,
      discover_timeout: parse_duration_secs(self.discover_timeout_secs),
      request_timeout: parse_duration_secs(self.request_timeout_secs),
    })
  }
}

fn cluster_identity(secret: Option<&str>, salt: Option<String>) -> Result<ClusterIdentity> {
  ClusterIdentity::from_secret_hex(
    secret,
    salt.map_or_else(default_cluster_salt, String::into_bytes),
  )
}

fn setup_logging() {
  tracing_subscriber::registry()
    .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
    .with(EnvFilter::from_default_env())
    .try_init()
    .ok();
}

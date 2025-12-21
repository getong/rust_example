use std::{
  collections::BTreeMap,
  path::{Path, PathBuf},
  time::Duration,
};

use anyhow::{Context, anyhow};
use clap::Parser;
use libp2p::{
  Multiaddr, PeerId, StreamProtocol, identity, noise,
  request_response::{self, ProtocolSupport},
  tcp, yamux,
};
use openraft::{BasicNode, Raft};
use tokio::sync::mpsc;

use crate::{
  network::{
    swarm::{Behaviour, Libp2pClient, run_swarm},
    transport::Libp2pNetworkFactory,
  },
  store,
  typ::NodeId,
};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about)]
pub struct Opt {
  /// Raft node id.
  #[arg(long)]
  pub id: u64,

  /// Libp2p listen address, e.g. /ip4/0.0.0.0/tcp/4001
  #[arg(long)]
  pub listen: String,

  /// Directory for RocksDB data.
  #[arg(long)]
  pub db: PathBuf,

  /// Path to persist libp2p identity (protobuf). Default: <db>/node.key
  #[arg(long)]
  pub key: Option<PathBuf>,

  /// Initialize cluster membership on startup.
  ///
  /// Provide all nodes (including self) with multiaddr including /p2p/<peerid>:
  ///   --init --node 1=/ip4/127.0.0.1/tcp/4001/p2p/12D3KooW... --node 2=...
  #[arg(long, default_value_t = false)]
  pub init: bool,

  /// Cluster node addresses in the form: <id>=<multiaddr-with-/p2p/peerid>
  #[arg(long = "node")]
  pub nodes: Vec<String>,
}

pub fn parse_node_kv(s: &str) -> anyhow::Result<(NodeId, String)> {
  let (id_str, addr) = s
    .split_once('=')
    .ok_or_else(|| anyhow!("expected <id>=<multiaddr>, got: {s}"))?;
  let id: NodeId = id_str.parse().context("invalid node id")?;
  Ok((id, addr.to_string()))
}

pub fn default_key_path(db_dir: &Path) -> PathBuf {
  db_dir.join("node.key")
}

pub fn load_or_create_keypair(path: &Path) -> anyhow::Result<identity::Keypair> {
  if let Ok(bytes) = std::fs::read(path) {
    let kp = identity::Keypair::from_protobuf_encoding(&bytes)
      .map_err(|e| anyhow!("invalid key file: {e}"))?;
    return Ok(kp);
  }

  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).ok();
  }

  let kp = identity::Keypair::generate_ed25519();
  let bytes = kp
    .to_protobuf_encoding()
    .map_err(|e| anyhow!("failed to encode keypair: {e}"))?;
  std::fs::write(path, bytes).context("write keypair")?;
  Ok(kp)
}

pub async fn run(opt: Opt) -> anyhow::Result<()> {
  std::fs::create_dir_all(&opt.db).context("create db dir")?;

  let key_path = opt.key.clone().unwrap_or_else(|| default_key_path(&opt.db));
  let local_key = load_or_create_keypair(&key_path)?;
  let local_peer_id = PeerId::from(local_key.public());
  tracing::info!("node_id={}, peer_id={}", opt.id, local_peer_id);

  let listen_addr: Multiaddr = opt.listen.parse().context("invalid --listen multiaddr")?;

  let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )
    .context("build tcp/noise/yamux")?
    .with_behaviour(|_| {
      let cfg = request_response::Config::default();
      Behaviour {
        raft: request_response::cbor::Behaviour::new(
          [(
            StreamProtocol::new("/openraft/raft/1"),
            ProtocolSupport::Full,
          )],
          cfg,
        ),
      }
    })
    .context("build behaviour")?
    .build();

  swarm.listen_on(listen_addr).context("listen_on")?;

  let config = openraft::Config {
    heartbeat_interval: 250,
    election_timeout_min: 299,
    ..Default::default()
  };
  let config = std::sync::Arc::new(config.validate().context("validate raft config")?);

  let (log_store, state_machine) = store::open_store(&opt.db).await?;

  let (cmd_tx, cmd_rx) = mpsc::channel(256);
  let client = Libp2pClient::new(cmd_tx.clone(), Duration::from_secs(5));
  let network = Libp2pNetworkFactory::new(client.clone());

  let raft = Raft::new(opt.id, config, network.clone(), log_store, state_machine)
    .await
    .context("create raft")?;

  tokio::spawn(run_swarm(swarm, cmd_rx, raft.clone()));

  let mut members: BTreeMap<NodeId, BasicNode> = BTreeMap::new();
  for n in &opt.nodes {
    let (id, addr) = parse_node_kv(n)?;
    network.register_node(id, &addr).await?;
    members.insert(
      id,
      BasicNode {
        addr: addr.to_string(),
      },
    );
  }

  if opt.init {
    if !members.contains_key(&opt.id) {
      return Err(anyhow!("--init requires providing self in --node list"));
    }
    tracing::info!("initializing cluster membership: {} nodes", members.len());
    let res = raft.initialize(members).await;
    tracing::info!("initialize result: {:?}", res);
  }

  futures::future::pending::<()>().await;
  Ok(())
}

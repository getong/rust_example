use std::{
  collections::BTreeMap,
  env,
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

const ENV_SELF_NAME: &str = "LIBP2P_SELF_NAME";
const ENV_BOOTSTRAP_NAME: &str = "LIBP2P_BOOTSTRAP_NAME";

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

fn load_env_file() {
  let candidates = [
    PathBuf::from(".env"),
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".env"),
  ];

  for path in candidates {
    let Ok(contents) = std::fs::read_to_string(&path) else {
      continue;
    };

    for raw_line in contents.lines() {
      let line = raw_line.trim();
      if line.is_empty() || line.starts_with('#') {
        continue;
      }

      let Some((key, value)) = line.split_once('=') else {
        continue;
      };

      let key = key.trim();
      if key.is_empty() || env::var_os(key).is_some() {
        continue;
      }

      let value = value.trim();
      unsafe {
        env::set_var(key, value);
      }
    }

    break;
  }
}

fn node_name_for_id(id: NodeId) -> String {
  let key = format!("LIBP2P_NODE_NAME_{id}");
  env::var(key).unwrap_or_else(|_| format!("node{id}"))
}

pub async fn run(opt: Opt) -> anyhow::Result<()> {
  load_env_file();

  std::fs::create_dir_all(&opt.db).context("create db dir")?;

  let key_path = opt.key.clone().unwrap_or_else(|| default_key_path(&opt.db));
  let local_key = load_or_create_keypair(&key_path)?;
  let local_peer_id = PeerId::from(local_key.public());
  let node_name = env::var(ENV_SELF_NAME).unwrap_or_else(|_| node_name_for_id(opt.id));
  tracing::info!(
    "node_id={}, node_name={}, peer_id={}",
    opt.id,
    node_name,
    local_peer_id
  );

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

  if let Ok(bootstrap_name) = env::var(ENV_BOOTSTRAP_NAME) {
    let mut bootstrap_target: Option<(NodeId, String)> = None;
    for (id, node) in &members {
      if node_name_for_id(*id) == bootstrap_name {
        bootstrap_target = Some((*id, node.addr.clone()));
        break;
      }
    }

    match bootstrap_target {
      Some((id, addr)) if id == opt.id => {
        tracing::info!(
          "bootstrap_name={}, bootstrap_id={}, bootstrap_addr={}, skipping self dial",
          bootstrap_name,
          id,
          addr
        );
      }
      Some((_id, addr)) => match addr.parse::<Multiaddr>() {
        Ok(maddr) => {
          tracing::info!("dialing bootstrap_name={} addr={}", bootstrap_name, addr);
          client.dial(maddr).await;
        }
        Err(err) => {
          tracing::warn!(
            "bootstrap_name={}, invalid multiaddr: {} ({})",
            bootstrap_name,
            addr,
            err
          );
        }
      },
      None => {
        tracing::warn!("bootstrap_name={} not found in --node list", bootstrap_name);
      }
    }
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

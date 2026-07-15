use std::{
  collections::{BTreeMap, HashMap, HashSet},
  env,
  net::SocketAddr,
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use anyhow::{Context, anyhow};
use clap::{ArgAction, Parser};
use futures::{AsyncRead, AsyncWrite};
use libp2p::{
  Multiaddr, PeerId, StreamProtocol, Swarm, Transport,
  core::upgrade::Version,
  dns, gossipsub, identity,
  kad::{self, store::MemoryStore},
  mdns, noise, ping,
  request_response::{self, ProtocolSupport},
  tcp, tls, websocket, yamux,
};
use openraft::{BasicNode, async_runtime::WatchReceiver};
use tokio::sync::mpsc;

use crate::{
  GroupHandle, GroupHandleMap, GroupId, NodeId,
  constants::{
    SERVICE_HTTP, SERVICE_LIBP2P_SWARM, SERVICE_OPENRAFT_LEADER_WORKER,
    SERVICE_SQLITE_CACHE_FLUSHER, SERVICE_TASK_WORKER,
  },
  groups, http, leader_controller,
  membership_guard::MembershipGuardConfig,
  network::{
    openraft_dispatcher::OpenRaftDispatcher,
    openraft_sync::OPENRAFT_SYNC_TOPIC,
    proto_codec::{ProstCodec, ProtoCodec, SerdeCodec},
    raft_bridge::P2PNetworkFactoryWrapper,
    rpc::{
      AddLearnerRequest, JoinClusterRequest, JoinClusterResponse, RaftRpcOp, RaftRpcRequest,
      RaftRpcResponse,
    },
    swarm::{
      Behaviour, Command, GOSSIP_TOPIC, KvClient, Libp2pClient, NODE_ANNOUNCE_TOPIC,
      OPENRAFT_CLUSTER_PROVIDER_KEY, SqliteSyncClient, TaskRpcClient, run_swarm,
    },
    transport::{Libp2pNetworkFactory, parse_p2p_addr},
  },
  openraft_group, openraft_groups,
  proto::raft_kv::{RaftKvRequest, RaftKvResponse},
  set_openraft_groups,
  sqlite_cache::{self, SqliteCache},
  sqlite_sync_rpc::{SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage},
  store,
  tasks::{
    self,
    rpc::{ControlNodes, TaskRpcRequestMessage, TaskRpcResponseMessage},
  },
  typ::Raft,
};

const ENV_SELF_NAME: &str = "LIBP2P_SELF_NAME";
const OPENRAFT_LEADER_CONTROLLER_INTERVAL_SECS: u64 = 1;
const MEMBERSHIP_GUARD_TICK_SECS: u64 = 5;
const EVICTED_LEARNER_REGISTER_RETRY_SECS: u64 = 10;
const SQLITE_CACHE_FLUSH_INTERVAL_SECS: u64 = 5;
const CONTROL_PROMOTION_POLL_INTERVAL_SECS: u64 = 2;
/// Poll cadence of the control demotion watcher (kad Server → Client when
/// this node is evicted from the voter set while still running).
const CONTROL_DEMOTION_POLL_INTERVAL_SECS: u64 = 30;
/// Consecutive confirmed "not a voter anywhere" rounds required before the
/// node demotes its kademlia role. Guards against transient membership
/// views during joint-consensus changes.
const CONTROL_DEMOTION_CONFIRMATIONS: u32 = 3;
const CONTROL_JOIN_POLL_INTERVAL_SECS: u64 = 2;
const CONTROL_JOIN_CATCH_UP_TIMEOUT_SECS: u64 = 30;
pub const OPENRAFT_KAD_PROVIDER_RECORD_TTL: Duration = Duration::from_secs(180);
pub const OPENRAFT_KAD_PROVIDER_PUBLICATION_INTERVAL: Duration = Duration::from_secs(60);
/// Kademlia's built-in periodic bootstrap cadence. Bootstraps also trigger
/// automatically (throttled) whenever a new peer enters the routing table,
/// so no manual bootstrap calls are needed anywhere.
pub const OPENRAFT_KAD_PERIODIC_BOOTSTRAP_INTERVAL: Duration = Duration::from_secs(60);
pub const PING_INTERVAL: Duration = Duration::from_secs(3);
pub const PING_TIMEOUT: Duration = Duration::from_secs(6);
const DEFAULT_MAX_CONTROL_NODES: usize = 3;
/// How long to wait for at least one remote peer to become connected before
/// running the startup "was this node removed?" membership check. Without
/// this wait the check always sees zero connected peers and silently skips,
/// missing the case where the node was evicted while it was offline.
const STARTUP_PEER_CONNECT_WAIT: Duration = Duration::from_secs(8);
/// Poll cadence of the post-startup openraft verification task.
const STARTUP_VERIFY_POLL_INTERVAL: Duration = Duration::from_secs(3);
/// How often the verification task warns while a group still has no leader.
const STARTUP_NO_LEADER_WARN_INTERVAL: Duration = Duration::from_secs(15);
/// Poll cadence of the known-nodes address book pruner.
const KNOWN_NODE_PRUNE_POLL_INTERVAL: Duration = Duration::from_secs(5);
/// How often each node announces itself on gossipsub so peers can (re)build
/// their known-nodes address book after prunes/restarts. This is the base
/// (small-cluster) interval; see [`adaptive_announce_interval`].
pub const NODE_ANNOUNCE_INTERVAL: Duration = Duration::from_secs(20);
/// Above this many known nodes the announce interval stretches
/// proportionally, capping the cluster-wide announce rate at roughly
/// `NODE_ANNOUNCE_SCALE_THRESHOLD / NODE_ANNOUNCE_INTERVAL` messages per
/// second no matter how large the cluster grows.
pub const NODE_ANNOUNCE_SCALE_THRESHOLD: usize = 64;
/// Upper bound on the stretched announce interval, so announce-based
/// liveness and post-prune re-listing stay bounded even in huge clusters.
pub const NODE_ANNOUNCE_MAX_INTERVAL: Duration = Duration::from_secs(300);

#[derive(Parser, Debug, Clone, Default)]
pub struct WebsocketOpt {
  /// Max websocket frame data size in bytes. Defaults to libp2p-websocket.
  #[arg(long)]
  pub ws_max_data_size: Option<usize>,

  /// Max websocket redirect hops to follow.
  #[arg(long)]
  pub ws_max_redirects: Option<u8>,

  /// Websocket TLS private key (DER, PKCS#8 or PKCS#1).
  #[arg(long)]
  pub ws_tls_key: Option<PathBuf>,

  /// Websocket TLS certificate chain (DER).
  #[arg(long)]
  pub ws_tls_cert: Option<PathBuf>,
}

pub fn apply_websocket_limits<T>(ws: &mut websocket::Config<T>, opt: &WebsocketOpt)
where
  T: Transport + Send + Unpin + 'static,
  T::Error: Send + 'static,
  T::Dial: Send + 'static,
  T::ListenerUpgrade: Send + 'static,
  T::Output: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
  if let Some(size) = opt.ws_max_data_size {
    ws.set_max_data_size(size);
  }
  if let Some(max) = opt.ws_max_redirects {
    ws.set_max_redirects(max);
  }
}

pub fn apply_websocket_tls<T>(
  ws: &mut websocket::Config<T>,
  opt: &WebsocketOpt,
) -> anyhow::Result<()>
where
  T: Transport + Send + Unpin + 'static,
  T::Error: Send + 'static,
  T::Dial: Send + 'static,
  T::ListenerUpgrade: Send + 'static,
  T::Output: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
  let Some(cert_path) = opt.ws_tls_cert.as_ref() else {
    if opt.ws_tls_key.is_some() {
      return Err(anyhow!("--ws-tls-key requires --ws-tls-cert"));
    }
    return Ok(());
  };

  let cert_bytes = std::fs::read(cert_path)
    .with_context(|| format!("read websocket TLS cert: {}", cert_path.display()))?;
  let cert = websocket::tls::Certificate::new(cert_bytes);

  // Create a custom TLS config that trusts our self-signed certificate
  let mut builder = websocket::tls::Config::builder();

  // Add our certificate as a trusted root for peer verification
  builder.add_trust(&cert)?;

  // If we have a private key, configure the server side
  if let Some(key_path) = opt.ws_tls_key.as_ref() {
    let key_bytes = std::fs::read(key_path)
      .with_context(|| format!("read websocket TLS key: {}", key_path.display()))?;
    let key = websocket::tls::PrivateKey::new(key_bytes);
    builder.server(key, vec![cert.clone()])?;
  }

  ws.set_tls_config(builder.finish());
  Ok(())
}

pub fn uses_wss(addr: &Multiaddr) -> bool {
  let mut saw_tls = false;
  for proto in addr.iter() {
    match proto {
      libp2p::multiaddr::Protocol::Wss(_) => return true,
      libp2p::multiaddr::Protocol::Tls => saw_tls = true,
      libp2p::multiaddr::Protocol::Ws(_) if saw_tls => return true,
      _ => {}
    }
  }
  false
}

pub fn build_ping_behaviour() -> ping::Behaviour {
  let config = ping::Config::new()
    .with_interval(PING_INTERVAL)
    .with_timeout(PING_TIMEOUT);
  ping::Behaviour::new(config)
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about)]
pub struct Opt {
  /// Raft node id. In the demo scripts this is the local libp2p PeerId.
  #[arg(long)]
  pub id: NodeId,

  /// Libp2p listen address, e.g. /ip4/0.0.0.0/tcp/4001/ws or /ip4/0.0.0.0/udp/4001/quic-v1
  #[arg(long)]
  pub listen: String,

  /// HTTP listen address for axum API.
  #[arg(long, default_value = "0.0.0.0:3000")]
  pub http: String,

  /// Directory for persistent storage data.
  #[arg(long)]
  pub db: PathBuf,

  /// Path to persist libp2p identity (protobuf). Default: <db>/node.key
  #[arg(long)]
  pub key: Option<PathBuf>,

  /// Known cluster node addresses in the form: <id>=<multiaddr-with-/p2p/peerid>.
  ///
  /// Kept for compatibility. Prefer --bootstrap-node for normal startup.
  #[arg(long = "node")]
  pub nodes: Vec<String>,

  /// Libp2p bootstrap node in the form: <id>=<multiaddr-with-/p2p/peerid>.
  ///
  /// If the local --id matches a bootstrap node id, this process bootstraps
  /// OpenRaft locally. Otherwise it dials the bootstrap node and requests to join.
  ///
  /// Non-bootstrap nodes dial this node and ask the current OpenRaft leader to
  /// join the control membership while it has fewer than --max-control-nodes voters.
  #[arg(long = "bootstrap-node")]
  pub bootstrap_nodes: Vec<String>,

  /// Address advertised in OpenRaft membership. Defaults to --listen with /p2p/<id> appended.
  #[arg(long)]
  pub advertise: Option<String>,

  /// Maximum number of OpenRaft control voters admitted by automatic startup join.
  #[arg(long, default_value_t = DEFAULT_MAX_CONTROL_NODES)]
  pub max_control_nodes: usize,

  /// OpenRaft heartbeat interval in milliseconds (leader keepalive cadence).
  ///
  /// Must be well below --raft-election-timeout-min-ms so that a few missed
  /// or delayed heartbeats (GC pause, swarm lock contention, RTT jitter) do
  /// not trigger a spurious election.
  #[arg(long, default_value_t = 500)]
  pub raft_keepalive_ms: u64,

  /// OpenRaft election timeout minimum in milliseconds.
  ///
  /// Followers wait a random duration in [min, max) before starting an
  /// election. Keep the [min, max) window wide (several heartbeat intervals)
  /// so simultaneous candidacies — and thus split votes — stay unlikely.
  #[arg(long, default_value_t = 1500)]
  pub raft_election_timeout_min_ms: u64,

  /// OpenRaft election timeout maximum in milliseconds.
  #[arg(long, default_value_t = 3000)]
  pub raft_election_timeout_max_ms: u64,

  /// Whether OpenRaft leader heartbeats are enabled.
  #[arg(long, default_value_t = true, action = ArgAction::Set)]
  pub raft_enable_heartbeat: bool,

  /// Disable tokio-console subscriber. It is enabled by default.
  #[arg(long)]
  pub no_tokio_console: bool,

  /// Redis URL used as the cache in front of SQLite.
  #[arg(long, default_value = "redis://127.0.0.1/")]
  pub redis_url: String,

  /// Disable Redis-backed SQLite cache integration.
  #[arg(long)]
  pub disable_sqlite_cache: bool,

  /// Close an idle libp2p connection only after this many seconds.
  #[arg(long, default_value_t = 30)]
  pub swarm_idle_connection_timeout_secs: u64,

  /// Automatically replace voters that stay unreachable with learners and
  /// backfill the learner pool from spare workers (runs on each group leader).
  #[arg(long, default_value_t = true, action = ArgAction::Set)]
  pub auto_heal_membership: bool,

  /// Seconds a member must stay unreachable before the membership guard acts
  /// on it: a dead voter is replaced with a learner, a dead learner is
  /// removed from the membership; both are backfilled from spare workers.
  #[arg(long, default_value_t = 300)]
  pub voter_replace_timeout_secs: u64,

  #[command(flatten)]
  pub websocket: WebsocketOpt,
}

pub fn parse_node_kv(s: &str) -> anyhow::Result<(NodeId, String)> {
  let (id_str, addr) = s
    .split_once('=')
    .ok_or_else(|| anyhow!("expected <id>=<multiaddr>, got: {s}"))?;
  let (peer, _) = parse_p2p_addr(addr)?;
  let peer_id = peer.to_string();
  if id_str != peer_id {
    return Err(anyhow!(
      "node id must match multiaddr /p2p peer id: id={id_str}, peer={peer_id}"
    ));
  }
  let id = NodeId::from(id_str);
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

fn node_name_for_id(id: &NodeId) -> String {
  let key = format!("LIBP2P_NODE_NAME_{id}");
  env::var(key).unwrap_or_else(|_| format!("node-{id}"))
}

#[derive(Clone)]
struct NodeIdentity {
  local_peer_id: PeerId,
  node_name: String,
}

#[derive(Clone)]
struct Libp2pHandles {
  cmd_tx: mpsc::Sender<Command>,
  client: Libp2pClient,
  kv_client: KvClient,
  network: Libp2pNetworkFactory,
}

#[derive(Clone)]
struct ControlRuntime {
  opt: Opt,
  identity: NodeIdentity,
  libp2p: Libp2pHandles,
  group_ids: Vec<GroupId>,
}

enum StartupMode {
  Worker { known_control_nodes: Vec<NodeId> },
  Control,
}

fn is_bootstrap_node(bootstrap_nodes: &[(NodeId, String)], self_id: &NodeId) -> bool {
  bootstrap_nodes.iter().any(|(id, _addr)| id == self_id)
}

fn init_node_identity(opt: &Opt) -> anyhow::Result<(identity::Keypair, NodeIdentity)> {
  let key_path = opt.key.clone().unwrap_or_else(|| default_key_path(&opt.db));
  let local_key = load_or_create_keypair(&key_path)?;
  let local_peer_id = PeerId::from(local_key.public());
  let local_peer_id_str = local_peer_id.to_string();
  if opt.id.to_string() != local_peer_id_str {
    return Err(anyhow!(
      "--id must match the local libp2p peer id from {}: expected {}, got {}",
      key_path.display(),
      local_peer_id_str,
      opt.id
    ));
  }
  let node_name = env::var(ENV_SELF_NAME).unwrap_or_else(|_| node_name_for_id(&opt.id));
  tracing::info!(
    "node_id={}, node_name={}, peer_id={}",
    opt.id,
    node_name,
    local_peer_id
  );
  Ok((
    local_key,
    NodeIdentity {
      local_peer_id,
      node_name,
    },
  ))
}

fn parse_listen_addr(opt: &Opt) -> anyhow::Result<Multiaddr> {
  let listen_addr: Multiaddr = opt.listen.parse().context("invalid --listen multiaddr")?;
  if uses_wss(&listen_addr)
    && (opt.websocket.ws_tls_key.is_none() || opt.websocket.ws_tls_cert.is_none())
  {
    return Err(anyhow!(
      "wss listen requires both --ws-tls-key and --ws-tls-cert"
    ));
  }
  Ok(listen_addr)
}

fn local_advertise_addr(opt: &Opt) -> anyhow::Result<String> {
  let addr = opt
    .advertise
    .clone()
    .unwrap_or_else(|| format!("{}/p2p/{}", opt.listen, opt.id));
  let (peer, _) = parse_p2p_addr(&addr).context("invalid --advertise multiaddr")?;
  if opt.id.as_str() != peer.to_string() {
    return Err(anyhow!(
      "--advertise /p2p peer id must match --id: id={}, peer={}",
      opt.id,
      peer
    ));
  }
  Ok(addr)
}

fn build_libp2p_handles(
  timeout: Duration,
  local_peer_id: PeerId,
) -> (Libp2pHandles, mpsc::Receiver<Command>) {
  let (cmd_tx, cmd_rx) = mpsc::channel(256);
  let client = Libp2pClient::new(cmd_tx.clone(), timeout);
  let kv_client = KvClient::new(cmd_tx.clone(), timeout);
  let sqlite_sync_client = SqliteSyncClient::new(cmd_tx.clone(), timeout);
  let task_rpc_client = TaskRpcClient::new(cmd_tx.clone(), timeout);
  let network = Libp2pNetworkFactory::new(
    client.clone(),
    kv_client.clone(),
    sqlite_sync_client.clone(),
    task_rpc_client,
    local_peer_id,
  );
  (
    Libp2pHandles {
      cmd_tx,
      client,
      kv_client,
      network,
    },
    cmd_rx,
  )
}

async fn start_openraft_groups(
  opt: &Opt,
  node_id: NodeId,
  db_dir: &Path,
  network: Libp2pNetworkFactory,
  group_ids: &[GroupId],
) -> anyhow::Result<GroupHandleMap> {
  if group_ids.is_empty() {
    return Err(anyhow!("no group ids configured"));
  }

  // openraft's own validate() only enforces heartbeat < min < max, which
  // still admits degenerate settings such as a 1ms randomization window or
  // an election timeout barely above the heartbeat interval. Both defeat
  // Raft's split-vote avoidance, so warn loudly when overridden that way.
  let randomization_window = opt
    .raft_election_timeout_max_ms
    .saturating_sub(opt.raft_election_timeout_min_ms);
  if randomization_window < opt.raft_keepalive_ms {
    tracing::warn!(
      election_timeout_min_ms = opt.raft_election_timeout_min_ms,
      election_timeout_max_ms = opt.raft_election_timeout_max_ms,
      "election timeout randomization window is narrower than one heartbeat interval; concurrent \
       candidacies are likely to split votes"
    );
  }
  if opt.raft_election_timeout_min_ms < opt.raft_keepalive_ms.saturating_mul(3) {
    tracing::warn!(
      heartbeat_interval_ms = opt.raft_keepalive_ms,
      election_timeout_min_ms = opt.raft_election_timeout_min_ms,
      "election timeout min is less than 3x the heartbeat interval; a single delayed heartbeat \
       can trigger a spurious election"
    );
  }

  let config = openraft::Config {
    heartbeat_interval: opt.raft_keepalive_ms,
    election_timeout_min: opt.raft_election_timeout_min_ms,
    election_timeout_max: opt.raft_election_timeout_max_ms,
    enable_heartbeat: opt.raft_enable_heartbeat,
    snapshot_policy: openraft::SnapshotPolicy::LogsSinceLast(1000),
    max_payload_entries: 200,
    purge_batch_size: 200,
    ..Default::default()
  };
  let config = Arc::new(config.validate().context("validate raft config")?);

  let mut groups = BTreeMap::new();

  for group_id in group_ids {
    let group_network = network.with_group(group_id.clone());
    let group_network = P2PNetworkFactoryWrapper::new(group_network);
    let (log_store, state_machine, kv_data) = store::open_store_for_group(db_dir, group_id).await?;

    let raft = Raft::new(
      node_id.clone(),
      config.clone(),
      group_network,
      log_store,
      state_machine,
    )
    .await
    .context("create raft")?;

    groups.insert(group_id.clone(), GroupHandle { raft, kv_data });
  }

  Ok(groups)
}

fn build_swarm(
  opt: &Opt,
  listen_addr: Multiaddr,
  local_key: identity::Keypair,
) -> anyhow::Result<Swarm<Behaviour>> {
  let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      (tls::Config::new, noise::Config::new),
      yamux::Config::default,
    )
    .context("build tcp/noise/yamux")?
    .with_quic()
    .with_other_transport(
      |key| -> Result<_, Box<dyn std::error::Error + Send + Sync>> {
        let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default());
        let dns_transport = dns::tokio::Transport::system(tcp_transport)?;
        let mut ws_transport = websocket::Config::new(dns_transport);
        apply_websocket_limits(&mut ws_transport, &opt.websocket);
        apply_websocket_tls(&mut ws_transport, &opt.websocket)
          .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;
        let security = noise::Config::new(key)?;
        Ok(
          ws_transport
            .upgrade(Version::V1Lazy)
            .authenticate(security)
            .multiplex(yamux::Config::default()),
        )
      },
    )
    .context("build websocket transport")?
    .with_behaviour(|key| {
      let cfg = request_response::Config::default();
      let peer_id = PeerId::from(key.public());
      let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?;
      let mut kad_config =
        kad::Config::new(StreamProtocol::new(crate::network::swarm::KAD_PROTOCOL));
      kad_config
        .set_provider_record_ttl(Some(OPENRAFT_KAD_PROVIDER_RECORD_TTL))
        .set_provider_publication_interval(Some(OPENRAFT_KAD_PROVIDER_PUBLICATION_INTERVAL))
        .set_periodic_bootstrap_interval(Some(OPENRAFT_KAD_PERIODIC_BOOTSTRAP_INTERVAL));
      let kad = kad::Behaviour::with_config(peer_id, MemoryStore::new(peer_id), kad_config);
      let gossipsub_config = crate::network::swarm::build_gossipsub_config()
        .map_err(|e| anyhow!("gossipsub config error: {e}"))?;
      let gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(key.clone()),
        gossipsub_config,
      )
      .map_err(|e| anyhow!("gossipsub init error: {e}"))?;
      let ping = build_ping_behaviour();

      Ok(Behaviour {
        raft_rpc: request_response::Behaviour::with_codec(
          ProtoCodec::default(),
          [(
            StreamProtocol::new(crate::network::swarm::RAFT_RPC_PROTOCOL),
            ProtocolSupport::Full,
          )],
          cfg.clone(),
        ),
        kv_rpc: request_response::Behaviour::with_codec(
          ProstCodec::<RaftKvRequest, RaftKvResponse>::default(),
          [(
            StreamProtocol::new(crate::network::swarm::KV_RPC_PROTOCOL),
            ProtocolSupport::Full,
          )],
          cfg.clone(),
        ),
        sqlite_sync_rpc: request_response::Behaviour::with_codec(
          SerdeCodec::<SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage>::default(),
          [(
            StreamProtocol::new(crate::network::swarm::SQLITE_SYNC_RPC_PROTOCOL),
            ProtocolSupport::Full,
          )],
          cfg.clone(),
        ),
        task_rpc: request_response::Behaviour::with_codec(
          SerdeCodec::<TaskRpcRequestMessage, TaskRpcResponseMessage>::default(),
          [(
            StreamProtocol::new(crate::network::swarm::TASK_RPC_PROTOCOL),
            ProtocolSupport::Full,
          )],
          cfg,
        ),
        gossipsub,
        ping,
        mdns,
        kad,
      })
    })
    .context("build behaviour")?
    .with_swarm_config(|cfg| {
      cfg
        .with_idle_connection_timeout(Duration::from_secs(opt.swarm_idle_connection_timeout_secs))
        .with_smart_dial()
    })
    .build();

  let gossip_topic = gossipsub::IdentTopic::new(GOSSIP_TOPIC);
  let announce_topic = gossipsub::IdentTopic::new(NODE_ANNOUNCE_TOPIC);
  let sync_topic = gossipsub::IdentTopic::new(OPENRAFT_SYNC_TOPIC);
  let sync_topic_hash = sync_topic.hash();
  swarm
    .behaviour_mut()
    .gossipsub
    .enable_partials_for_topic(sync_topic_hash, true);
  swarm
    .behaviour_mut()
    .gossipsub
    .subscribe(&sync_topic)
    .context("openraft sync gossipsub subscribe")?;
  swarm
    .behaviour_mut()
    .gossipsub
    .subscribe(&gossip_topic)
    .context("gossipsub subscribe")?;
  swarm
    .behaviour_mut()
    .gossipsub
    .subscribe(&announce_topic)
    .context("node announce gossipsub subscribe")?;

  swarm.listen_on(listen_addr).context("listen_on")?;
  Ok(swarm)
}

fn spawn_libp2p_swarm(
  swarm: Swarm<Behaviour>,
  shutdown: &mut crate::signal::ShutdownHandler,
  cmd_rx: mpsc::Receiver<Command>,
  libp2p: &Libp2pHandles,
) -> tokio::task::JoinHandle<()> {
  let swarm_done = shutdown.push(SERVICE_LIBP2P_SWARM);
  let swarm_shutdown = shutdown.shutdown_rx();
  let network_for_swarm = libp2p.network.clone();
  let dispatcher_for_swarm = Arc::new(OpenRaftDispatcher::new());
  let cmd_tx_for_swarm = libp2p.cmd_tx.clone();
  tokio::spawn(async move {
    run_swarm(
      swarm,
      cmd_rx,
      cmd_tx_for_swarm,
      network_for_swarm,
      dispatcher_for_swarm,
      swarm_shutdown,
    )
    .await;
    let _ = swarm_done.send(Ok(()));
  })
}

fn build_http_state(
  opt: &Opt,
  identity: &NodeIdentity,
  libp2p: &Libp2pHandles,
  sqlite_cache: Option<SqliteCache>,
  task_frontend: http::TaskFrontend,
) -> http::AppState {
  let default_group = default_openraft_group_id();

  http::AppState {
    node_id: opt.id.clone(),
    node_name: identity.node_name.clone(),
    peer_id: identity.local_peer_id.to_string(),
    listen: opt.listen.clone(),
    http_addr: opt.http.clone(),
    network: libp2p.network.clone(),
    kv_client: libp2p.kv_client.clone(),
    libp2p_client: libp2p.client.clone(),
    default_group,
    task_frontend,
    sqlite_cache,
  }
}

fn spawn_http(
  shutdown: &mut crate::signal::ShutdownHandler,
  http_addr: SocketAddr,
  http_state: http::AppState,
) -> tokio::task::JoinHandle<()> {
  let http_done = shutdown.push(SERVICE_HTTP);
  let http_shutdown = shutdown.shutdown_rx();
  tokio::spawn(async move {
    let res = http::serve(http_addr, http_state, http_shutdown).await;
    let _ = http_done.send(res);
  })
}

fn spawn_task_worker(
  shutdown: &mut crate::signal::ShutdownHandler,
  node_id: NodeId,
  worker_name: String,
  network: Libp2pNetworkFactory,
  control_nodes: Vec<NodeId>,
) -> tokio::task::JoinHandle<()> {
  let done = shutdown.push(SERVICE_TASK_WORKER);
  let worker_shutdown = shutdown.shutdown_rx();
  tokio::spawn(async move {
    let res = tasks::worker::run_task_worker(
      node_id,
      worker_name,
      groups::TASKS.to_string(),
      network,
      control_nodes,
      worker_shutdown,
    )
    .await;
    let _ = done.send(res);
  })
}

fn spawn_openraft_leader_controller(
  shutdown: &mut crate::signal::ShutdownHandler,
  groups: GroupHandleMap,
  network: Libp2pNetworkFactory,
  membership_guard_config: Option<MembershipGuardConfig>,
) -> tokio::task::JoinHandle<()> {
  let done = shutdown.push(SERVICE_OPENRAFT_LEADER_WORKER);
  let shutdown_rx = shutdown.shutdown_rx();
  tokio::spawn(async move {
    let res = leader_controller::run_leader_controller(
      groups,
      network,
      membership_guard_config,
      Duration::from_secs(OPENRAFT_LEADER_CONTROLLER_INTERVAL_SECS),
      shutdown_rx,
    )
    .await;
    let _ = done.send(res);
  })
}

fn linked_shutdown(parent_rx: crate::signal::ShutdownRx) -> crate::signal::ShutdownHandler {
  let (tx, rx) = crate::signal::channel();
  let tx_for_parent = tx.clone();
  let mut parent_rx = parent_rx;
  tokio::spawn(async move {
    let _ = parent_rx.changed().await;
    let _ = tx_for_parent.send(());
  });
  crate::signal::ShutdownHandler::new(tx, rx)
}

async fn run_control_services(
  runtime: ControlRuntime,
  http_addr: SocketAddr,
  shutdown_rx_for_ordering: crate::signal::ShutdownRx,
) -> anyhow::Result<()> {
  let sqlite_cache = if runtime.opt.disable_sqlite_cache {
    None
  } else {
    Some(SqliteCache::connect_in_db_dir(&runtime.opt.db, &runtime.opt.redis_url).await?)
  };
  if let Some(cache) = sqlite_cache.clone() {
    sqlite_cache::set_sqlite_cache(cache)
      .map_err(|_| anyhow!("global sqlite cache already initialized"))?;
  }
  let sqlite_flush_group_id = default_openraft_group_id();

  let leader_controller_groups = openraft_groups()
    .cloned()
    .ok_or_else(|| anyhow!("openraft groups are not initialized"))?;
  let http_state = build_http_state(
    &runtime.opt,
    &runtime.identity,
    &runtime.libp2p,
    sqlite_cache.clone(),
    http::TaskFrontend::Control,
  );

  let mut shutdown = linked_shutdown(shutdown_rx_for_ordering.clone());
  let _http_handle = spawn_http(&mut shutdown, http_addr, http_state);
  let membership_guard_config = runtime
    .opt
    .auto_heal_membership
    .then(|| MembershipGuardConfig {
      voter_replace_timeout: Duration::from_secs(runtime.opt.voter_replace_timeout_secs),
      tick_interval: Duration::from_secs(MEMBERSHIP_GUARD_TICK_SECS),
    });
  let _leader_controller_handle = spawn_openraft_leader_controller(
    &mut shutdown,
    leader_controller_groups,
    runtime.libp2p.network.clone(),
    membership_guard_config,
  );

  let _sqlite_flusher_handle = sqlite_cache.map(|_| {
    spawn_sqlite_cache_flusher(
      &mut shutdown,
      runtime.opt.id.clone(),
      sqlite_flush_group_id,
      runtime.libp2p.network.clone(),
      runtime.libp2p.kv_client.clone(),
    )
  });

  // Counterpart of the worker-side promotion watcher: demote the kademlia
  // role if this node gets evicted from the voter set while running.
  if runtime.opt.auto_heal_membership {
    tokio::spawn(run_control_demotion_watcher(
      runtime.clone(),
      shutdown_rx_for_ordering.clone(),
    ));
  }

  let (_tx, _rx, results) = shutdown.await_any_then_shutdown().await;

  let mut errors = Vec::new();
  for (service, res) in results {
    if let Err(err) = res {
      tracing::error!(service, error = ?err, "control service failed");
      errors.push(anyhow!("control service failed in {service}: {err}"));
    }
  }

  match errors.len() {
    0 => Ok(()),
    1 => Err(errors.remove(0)),
    _ => {
      let mut msg = String::new();
      use std::fmt::Write as _;
      let _ = writeln!(&mut msg, "control service errors: {}", errors.len());
      for err in errors {
        let _ = writeln!(&mut msg, "  {err}");
      }
      Err(anyhow!(msg))
    }
  }
}

/// Watch for this node being evicted from the voter set WHILE the process
/// keeps running (e.g. it was partitioned past the voter-replace timeout and
/// the membership guard replaced it, then connectivity came back).
///
/// Why this matters for kad: the control role advertises kademlia Server
/// mode plus the `openraft_cluster` provider record, and the provider record
/// is re-published every [`OPENRAFT_KAD_PROVIDER_PUBLICATION_INTERVAL`] —
/// so without this watcher an evicted-but-alive node keeps advertising
/// itself as a control node FOREVER and workers keep discovering it as a
/// task-RPC target. Raft itself is not affected by the kad mode (mode
/// changes never close connections; raft RPC uses the address book, not
/// kad lookups), which is exactly why nothing else notices the stale role.
///
/// Demotion is only performed on strong evidence: the LIVE leader of every
/// group (`state.is_leader()`, not the possibly-stale persisted
/// `current_leader`) must report that this node is not a voter, for
/// [`CONTROL_DEMOTION_CONFIRMATIONS`] consecutive rounds. On demotion the
/// node stops providing and drops to kad Client; a restart then completes
/// the self-heal (startup wipes the removed groups and rejoins as learner).
async fn run_control_demotion_watcher(
  runtime: ControlRuntime,
  mut shutdown_rx: crate::signal::ShutdownRx,
) {
  let mut tick = tokio::time::interval(Duration::from_secs(CONTROL_DEMOTION_POLL_INTERVAL_SECS));
  // If this very process was suspended (the main scenario that leads to
  // eviction), the default Burst behavior would fire all missed ticks
  // back-to-back on resume and collapse the confirmation rounds into one
  // instant. Delay keeps confirmations a full interval apart.
  tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
  tick.tick().await;
  let mut confirmations = 0u32;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => return,
      _ = tick.tick() => {}
    }

    if !confirmed_evicted_from_all_groups(&runtime).await {
      confirmations = 0;
      continue;
    }

    confirmations += 1;
    tracing::warn!(
      node_id = %runtime.opt.id,
      confirmations,
      required = CONTROL_DEMOTION_CONFIRMATIONS,
      "live leaders report this control node is no longer a voter"
    );
    if confirmations < CONTROL_DEMOTION_CONFIRMATIONS {
      continue;
    }

    match runtime.libp2p.client.leave_openraft_kad().await {
      Ok(()) => tracing::warn!(
        node_id = %runtime.opt.id,
        "node was evicted from the control membership while running; \
         dropped kademlia to Client mode and stopped providing the \
         control-node key. Restart this node to wipe stale group data \
         and rejoin as a learner."
      ),
      Err(err) => tracing::warn!(
        node_id = %runtime.opt.id,
        error = ?err,
        "failed to demote kademlia role after eviction"
      ),
    }
    return;
  }
}

/// True only when EVERY group has a live leader whose membership excludes
/// this node from the voter set. Unreachable groups or groups without a
/// live leader yield false — never demote on missing evidence.
async fn confirmed_evicted_from_all_groups(runtime: &ControlRuntime) -> bool {
  for group_id in &runtime.group_ids {
    let Some(metrics) =
      fetch_authoritative_group_metrics(group_id, &runtime.opt.id, &runtime.libp2p.network).await
    else {
      return false;
    };
    if metrics
      .membership_config
      .membership()
      .voter_ids()
      .any(|id| id == runtime.opt.id)
    {
      return false;
    }
  }
  true
}

enum PromotionWatchResult {
  Promote,
  Shutdown,
}

enum ControlJoinWatchResult {
  Joined,
  AlreadyMember,
  Full,
  Shutdown,
}

async fn run_control_promotion_watcher(
  runtime: ControlRuntime,
  mut shutdown_rx: crate::signal::ShutdownRx,
) -> anyhow::Result<PromotionWatchResult> {
  let mut tick = tokio::time::interval(Duration::from_secs(CONTROL_PROMOTION_POLL_INTERVAL_SECS));
  tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        return Ok(PromotionWatchResult::Shutdown);
      }
      _ = tick.tick() => {
        if remote_openraft_voters_contain_self(
          &runtime.opt.id,
          &runtime.group_ids,
          &runtime.libp2p.network,
        )
        .await
        {
          tracing::info!(
            node_id = %runtime.opt.id,
            "local node is now a voter in every OpenRaft group; promoting worker to control"
          );
          return Ok(PromotionWatchResult::Promote);
        }
      }
    }
  }
}

async fn run_control_join_watcher(
  runtime: ControlRuntime,
  bootstrap_nodes: Vec<NodeId>,
  advertise_addr: String,
  mut shutdown_rx: crate::signal::ShutdownRx,
) -> anyhow::Result<ControlJoinWatchResult> {
  if bootstrap_nodes.is_empty() || runtime.opt.max_control_nodes == 0 {
    return Ok(ControlJoinWatchResult::Full);
  }

  let mut tick = tokio::time::interval(Duration::from_secs(CONTROL_JOIN_POLL_INTERVAL_SECS));
  tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        return Ok(ControlJoinWatchResult::Shutdown);
      }
      _ = tick.tick() => {
        match try_join_control_cluster(&runtime, &bootstrap_nodes, &advertise_addr).await {
          Some(JoinClusterOutcome::Joined) => return Ok(ControlJoinWatchResult::Joined),
          Some(JoinClusterOutcome::AlreadyMember) => return Ok(ControlJoinWatchResult::AlreadyMember),
          Some(JoinClusterOutcome::Full) => return Ok(ControlJoinWatchResult::Full),
          None => {}
        }
      }
    }
  }
}

enum JoinClusterOutcome {
  Joined,
  AlreadyMember,
  Full,
}

/// Cap on per-group leader redirects while joining, so two groups whose
/// leaders keep moving cannot bounce the join loop forever.
const CONTROL_JOIN_MAX_LEADER_REDIRECTS: usize = 3;

async fn try_join_control_cluster(
  runtime: &ControlRuntime,
  bootstrap_nodes: &[NodeId],
  advertise_addr: &str,
) -> Option<JoinClusterOutcome> {
  for bootstrap_node in bootstrap_nodes {
    if bootstrap_node == &runtime.opt.id {
      continue;
    }

    let Some(metrics) = fetch_remote_group_metrics(
      &runtime.group_ids[0],
      &runtime.opt.id,
      &runtime.libp2p.network,
    )
    .await
    else {
      tracing::debug!(
        bootstrap_node = %bootstrap_node,
        "skip control join attempt because remote metrics are unavailable"
      );
      continue;
    };

    let membership = metrics.membership_config.membership();
    if membership.get_node(&runtime.opt.id).is_some() {
      // Being in the first group is not enough: joins run group by group, so
      // an interrupted attempt can leave this node in only a prefix of the
      // groups. Only report AlreadyMember when every group contains us;
      // otherwise fall through and finish joining the remaining groups.
      if remote_openraft_voters_contain_self(
        &runtime.opt.id,
        &runtime.group_ids,
        &runtime.libp2p.network,
      )
      .await
      {
        return Some(JoinClusterOutcome::AlreadyMember);
      }
      tracing::info!(
        node_id = %runtime.opt.id,
        "node is in a subset of raft groups; resuming control join for the remaining groups"
      );
    } else {
      let voter_count = membership.voter_ids().count();
      if voter_count >= runtime.opt.max_control_nodes {
        tracing::info!(
          voters = voter_count,
          max_voters = runtime.opt.max_control_nodes,
          "openraft control membership is full; staying worker"
        );
        return Some(JoinClusterOutcome::Full);
      }
    }

    let target_node = match metrics.current_leader.as_ref() {
      Some(leader_id) if leader_id != &runtime.opt.id => leader_id.clone(),
      _ => bootstrap_node.clone(),
    };

    match request_join_all_groups(runtime, target_node, advertise_addr).await {
      Ok(outcome) => return Some(outcome),
      Err(err) => {
        tracing::debug!(error = ?err, "control join request failed");
      }
    }
  }

  None
}

async fn request_join_all_groups(
  runtime: &ControlRuntime,
  target_node: NodeId,
  advertise_addr: &str,
) -> anyhow::Result<JoinClusterOutcome> {
  let mut joined_any = false;

  // Each raft group elects its own leader, so a single node rarely leads all
  // groups at once. Follow leader hints per group instead of restarting the
  // whole join with a new global target (which ping-pongs forever when two
  // groups have different leaders).
  for group_id in &runtime.group_ids {
    let mut group_target = target_node.clone();
    let mut redirects = 0;

    loop {
      let response =
        request_join_control_group(runtime, group_target.clone(), group_id, advertise_addr).await?;

      if response.already_member {
        break;
      }

      if response.joined {
        joined_any = true;
        break;
      }

      if let Some(leader_id) = response.leader_id.clone()
        && leader_id != group_target
        && leader_id != runtime.opt.id
        && redirects < CONTROL_JOIN_MAX_LEADER_REDIRECTS
      {
        if let Some(leader_addr) = response.leader_addr.as_deref() {
          let _ = runtime
            .libp2p
            .network
            .register_node(leader_id.clone(), leader_addr)
            .await;
        }
        tracing::debug!(
          group = group_id,
          leader = %leader_id,
          "redirecting control join to group leader"
        );
        group_target = leader_id;
        redirects += 1;
        continue;
      }

      if response.voter_count >= response.max_voters {
        tracing::info!(
          group = group_id,
          voters = response.voter_count,
          max_voters = response.max_voters,
          error = ?response.error,
          "openraft control membership is full; staying worker"
        );
        return Ok(JoinClusterOutcome::Full);
      }

      return Err(anyhow!(
        "join group {group_id} failed: {}",
        response
          .error
          .unwrap_or_else(|| "unknown error".to_string())
      ));
    }
  }

  Ok(if joined_any {
    JoinClusterOutcome::Joined
  } else {
    JoinClusterOutcome::AlreadyMember
  })
}

async fn request_join_control_group(
  runtime: &ControlRuntime,
  target_node: NodeId,
  group_id: &str,
  advertise_addr: &str,
) -> anyhow::Result<JoinClusterResponse> {
  let response = runtime
    .libp2p
    .network
    .request(
      target_node.clone(),
      RaftRpcRequest {
        group_id: group_id.to_string(),
        op: RaftRpcOp::JoinCluster(JoinClusterRequest {
          node_id: runtime.opt.id.clone(),
          addr: advertise_addr.to_string(),
          max_voters: runtime.opt.max_control_nodes,
          catch_up_timeout_ms: CONTROL_JOIN_CATCH_UP_TIMEOUT_SECS * 1000,
        }),
      },
    )
    .await?;

  match response {
    RaftRpcResponse::JoinCluster(response) => Ok(response),
    RaftRpcResponse::Error(message) => Err(anyhow!("join request failed: {message}")),
    other => Err(anyhow!("unexpected join response: {other:?}")),
  }
}

async fn run_worker_services_until_promotion(
  runtime: ControlRuntime,
  http_addr: SocketAddr,
  known_control_nodes: Vec<NodeId>,
  bootstrap_nodes: Vec<NodeId>,
  advertise_addr: String,
  evicted_from_cluster: bool,
  shutdown_rx: crate::signal::ShutdownRx,
) -> anyhow::Result<ControlJoinWatchResult> {
  let mut worker_shutdown = linked_shutdown(shutdown_rx.clone());
  let worker_shutdown_tx = worker_shutdown.shutdown_tx();
  let control_nodes_for_learner = known_control_nodes.clone();
  let advertise_addr_for_learner = advertise_addr.clone();

  let shared_control_nodes = Arc::new(tokio::sync::Mutex::new(ControlNodes::new(
    known_control_nodes.clone(),
  )));
  let http_state = build_http_state(
    &runtime.opt,
    &runtime.identity,
    &runtime.libp2p,
    None,
    http::TaskFrontend::Worker {
      control_nodes: shared_control_nodes,
    },
  );
  let worker_name = format!("libp2p-task-worker-{}", runtime.opt.id);
  let _http_handle = spawn_http(&mut worker_shutdown, http_addr, http_state);
  let _task_worker_handle = spawn_task_worker(
    &mut worker_shutdown,
    runtime.opt.id.clone(),
    worker_name,
    runtime.libp2p.network.clone(),
    known_control_nodes,
  );

  let mut worker_done_handle =
    tokio::spawn(async move { worker_shutdown.await_any_then_shutdown().await });
  let runtime_for_promotion = runtime.clone();
  let mut promotion_handle = tokio::spawn(run_control_promotion_watcher(
    runtime_for_promotion,
    shutdown_rx.clone(),
  ));
  let mut join_handle = tokio::spawn(run_control_join_watcher(
    runtime.clone(),
    bootstrap_nodes,
    advertise_addr,
    shutdown_rx.clone(),
  ));

  tokio::select! {
    promotion = &mut promotion_handle => {
      join_handle.abort();
      let _ = worker_shutdown_tx.send(());
      let (_tx, _rx, results) = worker_done_handle
        .await
        .map_err(|err| anyhow!("worker shutdown task failed: {err}"))?;
      collect_shutdown_errors("worker", results)?;

      match promotion.map_err(|err| anyhow!("promotion watcher task failed: {err}"))?? {
        PromotionWatchResult::Promote => Ok(ControlJoinWatchResult::AlreadyMember),
        PromotionWatchResult::Shutdown => Ok(ControlJoinWatchResult::Shutdown),
      }
    }
    join = &mut join_handle => {
      match join.map_err(|err| anyhow!("control join watcher task failed: {err}"))?? {
        ControlJoinWatchResult::Joined | ControlJoinWatchResult::AlreadyMember => {
          promotion_handle.abort();
          let _ = worker_shutdown_tx.send(());
          let (_tx, _rx, results) = worker_done_handle
            .await
            .map_err(|err| anyhow!("worker shutdown task failed: {err}"))?;
          collect_shutdown_errors("worker", results)?;
          Ok(ControlJoinWatchResult::Joined)
        }
        ControlJoinWatchResult::Full => {
          tracing::info!("automatic control join stopped; node remains a worker");
          // An evicted control node whose data was wiped re-joins the
          // cluster as a learner once the voter seats are taken.
          if evicted_from_cluster {
            tokio::spawn(run_evicted_learner_registration(
              runtime.clone(),
              control_nodes_for_learner,
              advertise_addr_for_learner,
              shutdown_rx.clone(),
            ));
          }
          match promotion_handle.await.map_err(|err| anyhow!("promotion watcher task failed: {err}"))?? {
            PromotionWatchResult::Promote => {
              let _ = worker_shutdown_tx.send(());
              let (_tx, _rx, results) = worker_done_handle
                .await
                .map_err(|err| anyhow!("worker shutdown task failed: {err}"))?;
              collect_shutdown_errors("worker", results)?;
              Ok(ControlJoinWatchResult::AlreadyMember)
            }
            PromotionWatchResult::Shutdown => {
              let (_tx, _rx, results) = worker_done_handle
                .await
                .map_err(|err| anyhow!("worker shutdown task failed: {err}"))?;
              collect_shutdown_errors("worker", results)?;
              Ok(ControlJoinWatchResult::Shutdown)
            }
          }
        }
        ControlJoinWatchResult::Shutdown => {
          promotion_handle.abort();
          let (_tx, _rx, results) = worker_done_handle
            .await
            .map_err(|err| anyhow!("worker shutdown task failed: {err}"))?;
          collect_shutdown_errors("worker", results)?;
          Ok(ControlJoinWatchResult::Shutdown)
        }
      }
    }
    worker_done = &mut worker_done_handle => {
      promotion_handle.abort();
      join_handle.abort();
      let (_tx, _rx, results) = worker_done
        .map_err(|err| anyhow!("worker shutdown task failed: {err}"))?;
      collect_shutdown_errors("worker", results)?;
      Ok(ControlJoinWatchResult::Shutdown)
    }
  }
}

/// Keep trying to register this (previously evicted) node as an OpenRaft
/// learner in every group until it succeeds or shutdown is requested.
async fn run_evicted_learner_registration(
  runtime: ControlRuntime,
  control_nodes: Vec<NodeId>,
  advertise_addr: String,
  mut shutdown_rx: crate::signal::ShutdownRx,
) {
  let mut tick = tokio::time::interval(Duration::from_secs(EVICTED_LEARNER_REGISTER_RETRY_SECS));

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => return,
      _ = tick.tick() => {
        match register_self_as_learner(&runtime, &control_nodes, &advertise_addr).await {
          Ok(()) => {
            tracing::info!(
              node_id = %runtime.opt.id,
              "evicted node re-registered as openraft learner in all groups"
            );
            return;
          }
          Err(err) => {
            tracing::debug!(
              node_id = %runtime.opt.id,
              error = ?err,
              "evicted learner registration attempt failed; retrying"
            );
          }
        }
      }
    }
  }
}

async fn register_self_as_learner(
  runtime: &ControlRuntime,
  control_nodes: &[NodeId],
  advertise_addr: &str,
) -> anyhow::Result<()> {
  for group_id in &runtime.group_ids {
    register_self_as_group_learner(runtime, control_nodes, group_id, advertise_addr).await?;
  }
  Ok(())
}

async fn register_self_as_group_learner(
  runtime: &ControlRuntime,
  control_nodes: &[NodeId],
  group_id: &str,
  advertise_addr: &str,
) -> anyhow::Result<()> {
  let mut last_error: Option<String> = None;

  for base_target in control_nodes {
    if base_target == &runtime.opt.id {
      continue;
    }
    let mut target = base_target.clone();

    for _redirect in 0 ..= CONTROL_JOIN_MAX_LEADER_REDIRECTS {
      let response = runtime
        .libp2p
        .network
        .request(
          target.clone(),
          RaftRpcRequest {
            group_id: group_id.to_string(),
            op: RaftRpcOp::AddLearner(AddLearnerRequest {
              node_id: runtime.opt.id.clone(),
              addr: advertise_addr.to_string(),
            }),
          },
        )
        .await;

      match response {
        Ok(RaftRpcResponse::AddLearner(resp)) if resp.ok => return Ok(()),
        Ok(RaftRpcResponse::AddLearner(resp)) => {
          if let Some(leader_id) = resp.leader_id
            && leader_id != target
            && leader_id != runtime.opt.id
          {
            if let Some(leader_addr) = resp.leader_addr.as_deref() {
              let _ = runtime
                .libp2p
                .network
                .register_node(leader_id.clone(), leader_addr)
                .await;
            }
            target = leader_id;
            continue;
          }
          last_error = resp.error;
          break;
        }
        Ok(RaftRpcResponse::Error(message)) => {
          last_error = Some(message);
          break;
        }
        Ok(other) => {
          last_error = Some(format!("unexpected add_learner response: {other:?}"));
          break;
        }
        Err(err) => {
          last_error = Some(format!("{err}"));
          break;
        }
      }
    }
  }

  Err(anyhow!(
    "register as learner for group {group_id} failed: {}",
    last_error.unwrap_or_else(|| "no reachable control node".to_string())
  ))
}

fn spawn_sqlite_cache_flusher(
  shutdown: &mut crate::signal::ShutdownHandler,
  local_node_id: NodeId,
  group_id: GroupId,
  network: Libp2pNetworkFactory,
  kv_client: KvClient,
) -> tokio::task::JoinHandle<()> {
  let done = shutdown.push(SERVICE_SQLITE_CACHE_FLUSHER);
  let shutdown_rx = shutdown.shutdown_rx();
  tokio::spawn(async move {
    sqlite_cache::run_sqlite_flush_worker(
      local_node_id,
      group_id,
      network,
      kv_client,
      Duration::from_secs(SQLITE_CACHE_FLUSH_INTERVAL_SECS),
      shutdown_rx,
    )
    .await;
    let _ = done.send(Ok(()));
  })
}

async fn register_members(
  network: &Libp2pNetworkFactory,
  nodes: &[(NodeId, String)],
) -> anyhow::Result<BTreeMap<NodeId, BasicNode>> {
  let mut members: BTreeMap<NodeId, BasicNode> = BTreeMap::new();
  for (id, addr) in nodes {
    network.register_node(id.clone(), &addr).await?;
    // Explicitly configured nodes (--node / --bootstrap-node) are the small
    // set the reconnect loop keeps permanently connected; everything learned
    // via gossip/mdns connects on demand instead.
    if let Ok((peer, _)) = parse_p2p_addr(addr) {
      network.pin_peer(peer).await;
    }
    members.insert(
      id.clone(),
      BasicNode {
        addr: addr.to_string(),
      },
    );
  }
  Ok(members)
}

fn configured_nodes(opt: &Opt) -> anyhow::Result<Vec<(NodeId, String)>> {
  let mut nodes = BTreeMap::new();
  for raw in opt.nodes.iter().chain(opt.bootstrap_nodes.iter()) {
    let (id, addr) = parse_node_kv(raw)?;
    nodes.insert(id, addr);
  }
  Ok(nodes.into_iter().collect())
}

fn known_control_nodes(members: &BTreeMap<NodeId, BasicNode>, self_id: &NodeId) -> Vec<NodeId> {
  members
    .keys()
    .filter(|node_id| *node_id != self_id)
    .cloned()
    .collect::<Vec<_>>()
}

/// True when this node is a VOTER of every raft group's persisted membership.
/// Learners are in `membership.nodes()` too, so a plain `get_node` check would
/// wrongly treat a learner as a control member.
fn local_openraft_voters_contain_self(
  db_dir: &Path,
  self_id: &NodeId,
  group_ids: &[GroupId],
) -> anyhow::Result<bool> {
  if group_ids.is_empty() {
    return Ok(false);
  }

  for group_id in group_ids {
    let Some(membership) = store::read_persisted_membership_for_group(db_dir, group_id)? else {
      return Ok(false);
    };
    if !membership.membership().voter_ids().any(|id| &id == self_id) {
      return Ok(false);
    }
  }
  Ok(true)
}

fn decide_startup_mode(
  opt: &Opt,
  members: &BTreeMap<NodeId, BasicNode>,
  bootstrap_nodes: &[(NodeId, String)],
  group_ids: &[GroupId],
) -> anyhow::Result<StartupMode> {
  if is_bootstrap_node(bootstrap_nodes, &opt.id) {
    return Ok(StartupMode::Control);
  }

  if local_openraft_voters_contain_self(&opt.db, &opt.id, group_ids)? {
    return Ok(StartupMode::Control);
  }

  Ok(StartupMode::Worker {
    known_control_nodes: known_control_nodes(members, &opt.id),
  })
}

/// True when this node is a VOTER of every raft group's remote membership.
///
/// This must NOT use `get_node`: learners are listed in `membership.nodes()`
/// as well, and the join flow (`add_learner` first, `change_membership`
/// later) would otherwise let the promotion watcher fire while the voter
/// promotion of later groups is still pending — aborting the join and
/// leaving the node as a learner in those groups forever.
async fn remote_openraft_voters_contain_self(
  self_id: &NodeId,
  group_ids: &[GroupId],
  network: &Libp2pNetworkFactory,
) -> bool {
  if group_ids.is_empty() {
    return false;
  }

  for group_id in group_ids {
    let Some(metrics) = fetch_remote_group_metrics(group_id, self_id, network).await else {
      return false;
    };
    if !metrics
      .membership_config
      .membership()
      .voter_ids()
      .any(|id| &id == self_id)
    {
      return false;
    }
  }
  true
}

/// Wait until at least one peer (other than ourselves) has an established
/// connection, or until `timeout` elapses. Returns `true` if a peer connected
/// within the window, `false` on timeout.
///
/// This is used before the startup membership-cleanup check so that the check
/// has a chance to reach remote nodes (dials are fire-and-forget and the
/// connection is established asynchronously).
async fn wait_for_any_peer_connected(network: &Libp2pNetworkFactory, timeout: Duration) -> bool {
  let deadline = tokio::time::Instant::now() + timeout;
  loop {
    for (_node_id, peer, _addr) in network.known_nodes().await {
      if network.is_peer_connected(&peer).await {
        return true;
      }
    }
    if tokio::time::Instant::now() >= deadline {
      return false;
    }
    tokio::time::sleep(Duration::from_millis(250)).await;
  }
}

/// Wipe local group data for groups where this node was removed from the
/// remote membership while offline. Returns `true` when any group data was
/// wiped, i.e. this node was evicted from the control membership.
async fn cleanup_removed_local_groups(
  db_dir: &Path,
  self_id: &NodeId,
  group_ids: &[GroupId],
  network: &Libp2pNetworkFactory,
) -> anyhow::Result<bool> {
  let mut wiped_any = false;
  for group_id in group_ids {
    let Some(local_membership) = store::read_persisted_membership_for_group(db_dir, group_id)?
    else {
      continue;
    };

    if local_membership.membership().get_node(self_id).is_none() {
      continue;
    }

    // Only an authoritative view (one that reports an elected leader) may
    // drive the destructive wipe. After a full-cluster restart, early-started
    // learners serve stale leaderless views that must not be trusted here.
    let Some(remote_metrics) = fetch_authoritative_group_metrics(group_id, self_id, network).await
    else {
      tracing::debug!(
        group = group_id,
        node_id = %self_id,
        "skip local data cleanup because no authoritative (leader-bearing) remote openraft metrics are available"
      );
      continue;
    };

    let remote_membership = remote_metrics.membership_config.membership();
    if remote_membership.nodes().next().is_none() {
      tracing::debug!(
        group = group_id,
        node_id = %self_id,
        "skip local data cleanup because remote openraft membership is empty"
      );
      continue;
    }

    if remote_membership.get_node(self_id).is_some() {
      continue;
    }

    tracing::warn!(
      group = group_id,
      node_id = %self_id,
      "local openraft data belongs to a node removed from remote membership; cleaning group data before startup"
    );
    store::remove_group_store(db_dir, group_id)?;
    wiped_any = true;
  }

  Ok(wiped_any)
}

async fn fetch_remote_group_metrics(
  group_id: &str,
  self_id: &NodeId,
  network: &Libp2pNetworkFactory,
) -> Option<crate::typ::RaftMetrics> {
  fetch_group_metrics_inner(group_id, self_id, network, false).await
}

/// Like [`fetch_remote_group_metrics`], but only accepts metrics served by a
/// node that is CURRENTLY the group leader (`state.is_leader()`), which is a
/// live runtime property. `current_leader` alone is not good enough: after a
/// restart openraft restores it from the persisted vote, so an early-started
/// learner can report the pre-restart leader even though no live leader
/// exists yet. Such stale views must not drive destructive decisions.
async fn fetch_authoritative_group_metrics(
  group_id: &str,
  self_id: &NodeId,
  network: &Libp2pNetworkFactory,
) -> Option<crate::typ::RaftMetrics> {
  fetch_group_metrics_inner(group_id, self_id, network, true).await
}

async fn fetch_group_metrics_inner(
  group_id: &str,
  self_id: &NodeId,
  network: &Libp2pNetworkFactory,
  require_leader: bool,
) -> Option<crate::typ::RaftMetrics> {
  for (node_id, _peer, _addr) in network.known_nodes().await {
    if &node_id == self_id {
      continue;
    }

    match network
      .request(
        node_id.clone(),
        RaftRpcRequest {
          group_id: group_id.to_string(),
          op: RaftRpcOp::GetMetrics,
        },
      )
      .await
    {
      Ok(RaftRpcResponse::GetMetrics(metrics)) => {
        if require_leader && !metrics.state.is_leader() {
          tracing::debug!(
            group = group_id,
            node_id = %node_id,
            "skipping remote openraft metrics view: responder is not the live leader"
          );
          continue;
        }
        return Some(metrics);
      }
      Ok(RaftRpcResponse::Error(message)) => {
        tracing::debug!(
          group = group_id,
          node_id = %node_id,
          error = %message,
          "remote openraft metrics request returned error"
        );
      }
      Ok(other) => {
        tracing::debug!(
          group = group_id,
          node_id = %node_id,
          response = ?other,
          "unexpected remote openraft metrics response"
        );
      }
      Err(err) => {
        tracing::debug!(
          group = group_id,
          node_id = %node_id,
          error = ?err,
          "remote openraft metrics request failed"
        );
      }
    }
  }

  None
}

/// Post-startup verification of the openraft groups. Runs until every group
/// reports an elected leader (then exits), logging progress so an
/// out-of-order full-cluster restart (learners up before the voters) is
/// visible instead of silently hanging. For nodes that booted in control
/// mode it also detects the "evicted while offline" case via an
/// authoritative remote view and shuts the node down so the next start can
/// wipe the stale data and re-join as a learner.
async fn run_openraft_startup_verifier(
  self_id: NodeId,
  group_ids: Vec<GroupId>,
  network: Libp2pNetworkFactory,
  boot_as_control: bool,
  shutdown_tx: crate::signal::ShutdownTx,
  mut shutdown_rx: crate::signal::ShutdownRx,
) {
  let started = tokio::time::Instant::now();
  let mut pending = group_ids;
  let mut last_warn = tokio::time::Instant::now();

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => return,
      _ = tokio::time::sleep(STARTUP_VERIFY_POLL_INTERVAL) => {}
    }

    let mut still_pending = Vec::new();
    for group_id in pending {
      let Some(group) = openraft_group(&group_id) else {
        still_pending.push(group_id);
        continue;
      };

      // Local leadership is live engine state, so it is trustworthy.
      // `current_leader` is NOT: openraft restores it from the persisted
      // vote, so after a restart it can name a leader that is still down.
      let local_is_leader = {
        let metrics_rx = group.raft.metrics();
        metrics_rx.borrow_watched().state.is_leader()
      };
      if local_is_leader {
        tracing::info!(
          group = %group_id,
          elapsed = ?started.elapsed(),
          "openraft group startup verified: this node is the live leader"
        );
        continue;
      }

      // Otherwise require a LIVE leader elsewhere: a remote node whose
      // metrics claim current leadership.
      if let Some(leader_metrics) =
        fetch_authoritative_group_metrics(&group_id, &self_id, &network).await
      {
        let membership = leader_metrics.membership_config.membership();
        let in_membership = membership.get_node(&self_id).is_some();

        // A control node that was evicted while offline can never rejoin
        // its stale config; restart cleanly so the next boot wipes the
        // stale data and re-joins as a learner.
        if boot_as_control && membership.nodes().next().is_some() && !in_membership {
          tracing::error!(
            group = %group_id,
            node_id = %self_id,
            "this control node was removed from the openraft membership while offline; \
             shutting down so the next start wipes the stale data and re-joins as a learner"
          );
          let _ = shutdown_tx.send(());
          return;
        }

        tracing::info!(
          group = %group_id,
          leader = %leader_metrics.id,
          in_membership,
          elapsed = ?started.elapsed(),
          "openraft group startup verified: live leader confirmed"
        );
        continue;
      }

      still_pending.push(group_id);
    }

    pending = still_pending;
    if pending.is_empty() {
      tracing::info!(
        elapsed = ?started.elapsed(),
        "openraft startup verification completed: all groups have a leader"
      );
      return;
    }

    if last_warn.elapsed() >= STARTUP_NO_LEADER_WARN_INTERVAL {
      last_warn = tokio::time::Instant::now();
      tracing::warn!(
        pending_groups = ?pending,
        elapsed = ?started.elapsed(),
        "openraft groups still have no leader; if this node started before the control voters, \
         the groups will recover automatically once a quorum of voters is online"
      );
    }
  }
}

/// Announce interval for a cluster with `known_nodes` entries in the address
/// book. Below [`NODE_ANNOUNCE_SCALE_THRESHOLD`] nodes this is the base
/// interval; above it the interval stretches proportionally so the
/// cluster-wide announce rate stays roughly constant (threshold / base
/// messages per second) instead of growing O(N). Capped so a returning node
/// is still re-listed within a bounded time.
pub fn adaptive_announce_interval(known_nodes: usize) -> Duration {
  let factor = known_nodes.div_ceil(NODE_ANNOUNCE_SCALE_THRESHOLD).max(1);
  NODE_ANNOUNCE_INTERVAL
    .saturating_mul(factor.min(u32::MAX as usize) as u32)
    .min(NODE_ANNOUNCE_MAX_INTERVAL)
}

/// Periodically announce this node's identity and advertise address on the
/// dedicated gossipsub topic. Peers use these announcements to (re)register
/// the node in their known-nodes address book — the reliable counterpart to
/// the pruner: a pruned node that comes back is re-listed within one
/// announce interval instead of waiting for a slow mdns re-discovery cycle.
///
/// The interval adapts to cluster size (see [`adaptive_announce_interval`])
/// and each announcement carries it, so receivers scale the sender's
/// liveness TTL to the actual cadence instead of assuming the base one.
async fn run_node_announcer(
  self_id: NodeId,
  advertise_addr: String,
  network: Libp2pNetworkFactory,
  kv_client: KvClient,
  mut shutdown_rx: crate::signal::ShutdownRx,
) {
  use prost::Message as _;

  loop {
    let interval = adaptive_announce_interval(network.known_nodes_count().await);
    let announcement = crate::proto::raft_kv::NodeAnnouncement {
      node_id: self_id.to_string(),
      addr: advertise_addr.clone(),
      ts_unix_ms: std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default(),
      announce_interval_ms: interval.as_millis() as u64,
    };
    kv_client
      .publish_gossipsub(NODE_ANNOUNCE_TOPIC, announcement.encode_to_vec())
      .await;

    tokio::select! {
      _ = shutdown_rx.changed() => return,
      _ = tokio::time::sleep(interval) => {}
    }
  }
}

/// Prune nodes that stay disconnected past `prune_timeout` from the libp2p
/// known-nodes address book, so crashed non-member workers do not linger in
/// `/cluster` forever. Raft group members are exempt: their lifecycle belongs
/// to the membership guard, and only after the guard has removed them from
/// every group does the pruner start counting for them.
async fn run_known_nodes_pruner(
  self_id: NodeId,
  network: Libp2pNetworkFactory,
  prune_timeout: Duration,
  mut shutdown_rx: crate::signal::ShutdownRx,
) {
  let mut down_since: HashMap<NodeId, tokio::time::Instant> = HashMap::new();
  let mut tick = tokio::time::interval(KNOWN_NODE_PRUNE_POLL_INTERVAL);
  tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => return,
      _ = tick.tick() => {}
    }

    let now = tokio::time::Instant::now();
    let mut present: HashSet<NodeId> = HashSet::new();

    for (node_id, peer_id, _addr) in network.known_nodes().await {
      if node_id == self_id {
        continue;
      }
      present.insert(node_id.clone());

      // Alive = connected OR announcing recently; with on-demand connections
      // a healthy idle node is intentionally not connected to us.
      if network.is_peer_alive(&peer_id).await {
        down_since.remove(&node_id);
        continue;
      }

      // Members (voters/learners) are handled by the membership guard.
      if is_openraft_member_of_any_group(&node_id) {
        down_since.remove(&node_id);
        continue;
      }

      let since = *down_since.entry(node_id.clone()).or_insert(now);
      let downtime = now.duration_since(since);
      if downtime >= prune_timeout {
        if network.remove_known_node(&node_id).await {
          tracing::warn!(
            node_id = %node_id,
            downtime = ?downtime,
            "pruned dead libp2p node from the known-nodes address book"
          );
        }
        down_since.remove(&node_id);
      }
    }

    down_since.retain(|id, _| present.contains(id));
  }
}

fn is_openraft_member_of_any_group(node_id: &NodeId) -> bool {
  let Some(groups) = openraft_groups() else {
    return false;
  };
  for group in groups.values() {
    let metrics_rx = group.raft.metrics();
    let is_member = metrics_rx
      .borrow_watched()
      .membership_config
      .membership()
      .get_node(node_id)
      .is_some();
    if is_member {
      return true;
    }
  }
  false
}

async fn maybe_bootstrap(
  client: &Libp2pClient,
  bootstrap_nodes: &[(NodeId, String)],
  self_id: &NodeId,
) {
  if bootstrap_nodes.is_empty() {
    return;
  }

  for (id, addr) in bootstrap_nodes {
    if id == self_id {
      tracing::info!(
        "bootstrap_id={}, bootstrap_addr={}, skipping self dial",
        id,
        addr
      );
      continue;
    }

    match addr.parse::<Multiaddr>() {
      Ok(maddr) => {
        tracing::info!("dialing bootstrap_id={} addr={}", id, addr);
        client.dial(maddr).await;
      }
      Err(err) => {
        tracing::warn!("bootstrap_id={}, invalid multiaddr: {} ({})", id, addr, err);
      }
    }
  }
}

async fn maybe_initialize_bootstrap_openraft(
  self_id: NodeId,
  self_addr: String,
  bootstrap_self: bool,
) -> anyhow::Result<()> {
  if !bootstrap_self {
    return Ok(());
  }

  let members = BTreeMap::from([(self_id, BasicNode { addr: self_addr })]);
  let groups = openraft_groups()
    .map(|groups| {
      groups
        .iter()
        .map(|(group_id, group)| (group_id.clone(), group.raft.clone()))
        .collect::<Vec<_>>()
    })
    .ok_or_else(|| anyhow!("openraft groups are not initialized"))?;
  tracing::info!(
    "initializing cluster membership: {} nodes, {} groups",
    members.len(),
    groups.len()
  );
  for (group_id, raft) in groups {
    let res = raft.initialize(members.clone()).await;
    tracing::info!(group = group_id, "initialize result: {:?}", res);
  }
  Ok(())
}

fn default_openraft_group_id() -> GroupId {
  if openraft_groups().is_some_and(|raft_groups| raft_groups.contains_key(groups::USERS)) {
    return groups::USERS.to_string();
  }

  openraft_groups()
    .and_then(|raft_groups| raft_groups.keys().next().cloned())
    .unwrap_or_else(|| groups::USERS.to_string())
}

fn collect_shutdown_errors(
  label: &str,
  results: Vec<(&'static str, anyhow::Result<()>)>,
) -> anyhow::Result<()> {
  let mut errors = Vec::new();
  for (service, res) in results {
    if let Err(err) = res {
      tracing::error!(service, error = ?err, "shutdown task failed");
      errors.push((service, err));
    }
  }

  if errors.is_empty() {
    return Ok(());
  }

  if errors.len() == 1 {
    let (service, err) = errors.pop().unwrap();
    return Err(anyhow!("{label} error in {service}: {err}"));
  }

  let mut msg = String::new();
  use std::fmt::Write as _;
  let _ = writeln!(&mut msg, "{label} encountered {} errors:", errors.len());
  for (service, err) in errors {
    let _ = writeln!(&mut msg, "  {service}: {err}");
  }
  Err(anyhow!(msg))
}

async fn advertise_openraft_kad(client: &Libp2pClient) {
  client.set_kad_mode(kad::Mode::Server).await;
  if let Err(e) = client
    .start_providing(OPENRAFT_CLUSTER_PROVIDER_KEY.to_string())
    .await
  {
    tracing::warn!(
      "failed to provide {} capability via Kademlia: {:?}",
      OPENRAFT_CLUSTER_PROVIDER_KEY,
      e
    );
  }
}

async fn leave_openraft_kad(client: &Libp2pClient) -> anyhow::Result<()> {
  match client.leave_openraft_kad().await {
    Ok(()) => {
      tracing::info!(
        provider_key = OPENRAFT_CLUSTER_PROVIDER_KEY,
        "left openraft kademlia provider before openraft shutdown"
      );
      Ok(())
    }
    Err(err) => {
      tracing::warn!(
        provider_key = OPENRAFT_CLUSTER_PROVIDER_KEY,
        error = ?err,
        "could not leave openraft kademlia provider; swarm may already be stopped"
      );
      Err(anyhow!("leave openraft kademlia provider failed: {err}"))
    }
  }
}

async fn shutdown_openraft_groups() -> anyhow::Result<()> {
  let Some(rafts) = openraft_groups().map(|groups| {
    groups
      .iter()
      .map(|(group_id, group)| (group_id.clone(), group.raft.clone()))
      .collect::<Vec<_>>()
  }) else {
    tracing::info!("openraft groups are not initialized; skip openraft shutdown");
    return Ok(());
  };

  tracing::info!(
    groups = rafts.len(),
    "shutdown phase: stopping openraft groups and waiting"
  );

  let mut errors = Vec::new();
  for (group_id, raft) in rafts {
    tracing::info!(group = %group_id, "openraft group shutdown started");
    match raft.shutdown().await {
      Ok(()) => {
        tracing::info!(
          group = %group_id,
          "openraft group shutdown completed; raft storage writes use sync wal"
        );
      }
      Err(err) => {
        tracing::error!(
          group = %group_id,
          error = ?err,
          "openraft group shutdown failed"
        );
        errors.push(anyhow!(
          "openraft group {group_id} shutdown failed: {err:?}"
        ));
      }
    }
  }

  match errors.len() {
    0 => Ok(()),
    1 => Err(errors.remove(0)),
    _ => {
      let mut msg = String::new();
      use std::fmt::Write as _;
      let _ = writeln!(&mut msg, "openraft shutdown errors: {}", errors.len());
      for err in errors {
        let _ = writeln!(&mut msg, "  {err}");
      }
      Err(anyhow!(msg))
    }
  }
}

async fn run_graceful_shutdown(
  libp2p_client: &Libp2pClient,
  libp2p_shutdown: crate::signal::ShutdownHandler,
  swarm_handle: tokio::task::JoinHandle<()>,
) -> anyhow::Result<()> {
  let shutdown_started = tokio::time::Instant::now();
  let mut errors = Vec::new();

  tracing::info!("shutdown phase: leaving libp2p kademlia provider mode");
  if let Err(err) = leave_openraft_kad(libp2p_client).await {
    tracing::error!(error = ?err, "shutdown phase failed: leave libp2p kademlia");
    errors.push(err);
  }

  tracing::info!("shutdown phase: stopping libp2p swarm and waiting");
  let (_tx, _rx, results) = libp2p_shutdown.shutdown().await;
  if let Err(err) = collect_shutdown_errors("libp2p shutdown", results) {
    tracing::error!(error = ?err, "shutdown phase failed: libp2p service");
    errors.push(err);
  }

  match swarm_handle.await {
    Ok(()) => {
      tracing::info!("shutdown phase: libp2p swarm task joined");
    }
    Err(err) => {
      tracing::error!(error = ?err, "shutdown phase failed: libp2p swarm task join");
      errors.push(anyhow!("libp2p swarm task failed: {err}"));
    }
  }

  if let Err(err) = shutdown_openraft_groups().await {
    tracing::error!(error = ?err, "shutdown phase failed: openraft");
    errors.push(err);
  }

  if errors.is_empty() {
    tracing::info!(
      elapsed = ?shutdown_started.elapsed(),
      "graceful shutdown phases completed"
    );
    return Ok(());
  }

  combine_errors("graceful shutdown", errors)
}

fn combine_errors(label: &str, mut errors: Vec<anyhow::Error>) -> anyhow::Result<()> {
  if errors.is_empty() {
    return Ok(());
  }

  if errors.len() == 1 {
    return Err(errors.remove(0));
  }

  let mut msg = String::new();
  use std::fmt::Write as _;
  let _ = writeln!(&mut msg, "{label} encountered {} errors:", errors.len());
  for err in errors {
    let _ = writeln!(&mut msg, "  {err}");
  }
  Err(anyhow!(msg))
}

pub async fn run(opt: Opt) -> anyhow::Result<()> {
  load_env_file();
  let http_addr: SocketAddr = opt.http.parse().context("invalid --http")?;

  std::fs::create_dir_all(&opt.db).context("create db dir")?;

  let (local_key, identity) = init_node_identity(&opt)?;
  let listen_addr = parse_listen_addr(&opt)?;

  let timeout = Duration::from_secs(5);
  let (libp2p, cmd_rx) = build_libp2p_handles(timeout, identity.local_peer_id.clone());

  let group_ids = groups::all();
  let advertise_addr = local_advertise_addr(&opt)?;
  let configured_nodes = configured_nodes(&opt)?;
  let configured_bootstrap_nodes = opt
    .bootstrap_nodes
    .iter()
    .map(|node| parse_node_kv(node))
    .collect::<anyhow::Result<Vec<_>>>()?;
  if configured_bootstrap_nodes.is_empty() {
    return Err(anyhow!(
      "--bootstrap-node is required; set it to the bootstrap node's <id>=<multiaddr>"
    ));
  }
  let bootstrap_self = is_bootstrap_node(&configured_bootstrap_nodes, &opt.id);
  let bootstrap_nodes = opt
    .bootstrap_nodes
    .iter()
    .map(|node| parse_node_kv(node).map(|(id, _addr)| id))
    .collect::<anyhow::Result<Vec<_>>>()?;

  let swarm = build_swarm(&opt, listen_addr, local_key)?;
  let signal_shutdown = crate::signal::spawn_handler();
  let shutdown_rx_for_ordering = signal_shutdown.shutdown_rx();
  let (libp2p_shutdown_tx, libp2p_shutdown_rx) = crate::signal::channel();
  let mut libp2p_shutdown =
    crate::signal::ShutdownHandler::new(libp2p_shutdown_tx, libp2p_shutdown_rx);

  let swarm_handle = spawn_libp2p_swarm(swarm, &mut libp2p_shutdown, cmd_rx, &libp2p);

  let members = register_members(&libp2p.network, &configured_nodes).await?;
  maybe_bootstrap(&libp2p.client, &configured_bootstrap_nodes, &opt.id).await;

  // Dials issued by register_members are fire-and-forget; wait a short window
  // for at least one peer connection to be established before running the
  // removed-node check, so the check can actually reach remote nodes.
  if !configured_nodes.is_empty() {
    let peer_connected =
      wait_for_any_peer_connected(&libp2p.network, STARTUP_PEER_CONNECT_WAIT).await;
    if !peer_connected {
      tracing::warn!(
        "no peer connected within {:?}; removed-node detection may be skipped",
        STARTUP_PEER_CONNECT_WAIT
      );
    }
  }

  let mut evicted_from_cluster = false;
  if !bootstrap_self {
    evicted_from_cluster =
      cleanup_removed_local_groups(&opt.db, &opt.id, &group_ids, &libp2p.network).await?;
  }
  let startup_mode = decide_startup_mode(&opt, &members, &configured_bootstrap_nodes, &group_ids)?;

  let group_handles = start_openraft_groups(
    &opt,
    opt.id.clone(),
    &opt.db,
    libp2p.network.clone(),
    &group_ids,
  )
  .await?;
  set_openraft_groups(group_handles)
    .map_err(|_| anyhow!("global openraft groups already initialized"))?;
  maybe_initialize_bootstrap_openraft(opt.id.clone(), advertise_addr.clone(), bootstrap_self)
    .await?;

  // Verify that every openraft group actually becomes operational after this
  // (possibly out-of-order) startup, and self-correct evicted control nodes.
  tokio::spawn(run_openraft_startup_verifier(
    opt.id.clone(),
    group_ids.clone(),
    libp2p.network.clone(),
    matches!(startup_mode, StartupMode::Control),
    signal_shutdown.shutdown_tx(),
    signal_shutdown.shutdown_rx(),
  ));

  // Prune long-dead non-member nodes from the libp2p address book.
  if opt.auto_heal_membership {
    tokio::spawn(run_known_nodes_pruner(
      opt.id.clone(),
      libp2p.network.clone(),
      Duration::from_secs(opt.voter_replace_timeout_secs),
      signal_shutdown.shutdown_rx(),
    ));
  }

  // Announce this node so peers can (re)register it after prunes/restarts.
  tokio::spawn(run_node_announcer(
    opt.id.clone(),
    advertise_addr.clone(),
    libp2p.network.clone(),
    libp2p.kv_client.clone(),
    signal_shutdown.shutdown_rx(),
  ));

  let runtime = ControlRuntime {
    opt,
    identity,
    libp2p,
    group_ids,
  };
  let libp2p_client_for_shutdown = runtime.libp2p.client.clone();

  let service_result = match startup_mode {
    StartupMode::Control => {
      advertise_openraft_kad(&runtime.libp2p.client).await;
      run_control_services(runtime, http_addr, shutdown_rx_for_ordering).await
    }
    StartupMode::Worker {
      known_control_nodes,
    } => {
      runtime.libp2p.client.set_kad_mode(kad::Mode::Client).await;
      let mut all_control_nodes = known_control_nodes;
      if let Ok(providers) = runtime
        .libp2p
        .client
        .get_providers(OPENRAFT_CLUSTER_PROVIDER_KEY.to_string())
        .await
      {
        for peer in providers {
          all_control_nodes.push(NodeId::new(peer.to_string()));
        }
      }
      all_control_nodes.sort();
      all_control_nodes.dedup();
      match run_worker_services_until_promotion(
        runtime.clone(),
        http_addr,
        all_control_nodes,
        bootstrap_nodes,
        advertise_addr,
        evicted_from_cluster,
        shutdown_rx_for_ordering.clone(),
      )
      .await
      {
        Ok(ControlJoinWatchResult::Joined | ControlJoinWatchResult::AlreadyMember) => {
          advertise_openraft_kad(&runtime.libp2p.client).await;
          run_control_services(runtime, http_addr, shutdown_rx_for_ordering).await
        }
        Ok(ControlJoinWatchResult::Full | ControlJoinWatchResult::Shutdown) => Ok(()),
        Err(err) => Err(err),
      }
    }
  };

  if let Err(err) = service_result.as_ref() {
    tracing::error!(
      error = ?err,
      "service loop exited with error; continuing graceful shutdown"
    );
  } else {
    tracing::info!("service loop exited; starting graceful shutdown");
  }

  let graceful_shutdown_result =
    run_graceful_shutdown(&libp2p_client_for_shutdown, libp2p_shutdown, swarm_handle).await;

  let mut final_errors = Vec::new();
  if let Err(err) = service_result {
    final_errors.push(anyhow!("service loop failed: {err}"));
  }
  if let Err(err) = graceful_shutdown_result {
    final_errors.push(err);
  }
  combine_errors("application shutdown", final_errors)?;

  tracing::info!("shutdown complete");
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn announce_interval_stays_base_for_small_clusters() {
    assert_eq!(adaptive_announce_interval(0), NODE_ANNOUNCE_INTERVAL);
    assert_eq!(adaptive_announce_interval(1), NODE_ANNOUNCE_INTERVAL);
    assert_eq!(
      adaptive_announce_interval(NODE_ANNOUNCE_SCALE_THRESHOLD),
      NODE_ANNOUNCE_INTERVAL
    );
  }

  #[test]
  fn announce_interval_scales_linearly_with_cluster_size() {
    assert_eq!(
      adaptive_announce_interval(NODE_ANNOUNCE_SCALE_THRESHOLD + 1),
      NODE_ANNOUNCE_INTERVAL * 2
    );
    assert_eq!(
      adaptive_announce_interval(NODE_ANNOUNCE_SCALE_THRESHOLD * 4),
      NODE_ANNOUNCE_INTERVAL * 4
    );
  }

  #[test]
  fn announce_interval_is_capped() {
    assert_eq!(
      adaptive_announce_interval(1_000_000),
      NODE_ANNOUNCE_MAX_INTERVAL
    );
  }
}

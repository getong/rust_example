use std::{
  collections::{BTreeMap, BTreeSet},
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
  Multiaddr, PeerId, StreamProtocol, Transport,
  core::upgrade::Version,
  dns, gossipsub, identity,
  kad::{self, store::MemoryStore},
  mdns, noise, ping,
  request_response::{self, ProtocolSupport},
  tcp, tls, websocket, yamux,
};
use openraft::{BasicNode, ChangeMembers, ServerState, async_runtime::WatchReceiver};
use rand::seq::IndexedRandom;
use tokio::sync::mpsc;

use crate::{
  GroupHandle, GroupHandleMap, GroupId, NodeId, apalis_raft,
  constants::{
    SERVICE_APALIS_WORKER, SERVICE_HTTP, SERVICE_LIBP2P_SWARM, SERVICE_OPENRAFT,
    SERVICE_OPENRAFT_AUTOSCALER, SERVICE_SQLITE_CACHE_FLUSHER,
  },
  groups, http,
  network::{
    openraft_dispatcher::OpenRaftDispatcher,
    openraft_sync::OPENRAFT_SYNC_TOPIC,
    proto_codec::{ProstCodec, ProtoCodec, SerdeCodec},
    raft_bridge::P2PNetworkFactoryWrapper,
    rpc::{RaftRpcOp, RaftRpcRequest, RaftRpcResponse},
    swarm::{
      Behaviour, Command, GOSSIP_TOPIC, KvClient, Libp2pClient, SqliteSyncClient, run_swarm,
      set_libp2p_swarm,
    },
    transport::{Libp2pNetworkFactory, parse_p2p_addr},
  },
  openraft_group, openraft_groups,
  proto::raft_kv::{RaftKvRequest, RaftKvResponse},
  set_openraft_groups,
  sqlite_cache::{self, SqliteCache},
  sqlite_sync_rpc::{SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage},
  store,
  typ::Raft,
};

const ENV_SELF_NAME: &str = "LIBP2P_SELF_NAME";
const ENV_BOOTSTRAP_NAME: &str = "LIBP2P_BOOTSTRAP_NAME";
const OPENRAFT_MAX_LEARNERS: usize = 5;
const OPENRAFT_MAX_VOTERS: usize = 5;
const OPENRAFT_AUTOSCALER_INTERVAL_SECS: u64 = 5;
const SQLITE_CACHE_FLUSH_INTERVAL_SECS: u64 = 5;
const OPENRAFT_OFFLINE_REMOVE_AFTER_SECS: u64 = 300;
/// Max log-index lag a learner may have compared to the leader before being
/// eligible for promotion to voter. A learner further behind than this is
/// skipped until it catches up, preventing a lagging node from being promoted
/// into the quorum where it would slow down commit latency.
const LEARNER_PROMOTE_MAX_LAG: u64 = 500;
/// How long to wait for at least one remote peer to become connected before
/// running the startup "was this node removed?" membership check. Without
/// this wait the check always sees zero connected peers and silently skips,
/// missing the case where the node was evicted while it was offline.
const STARTUP_PEER_CONNECT_WAIT: Duration = Duration::from_secs(8);
/// Timeout for a single graceful-leave attempt on one Raft group at shutdown.
const GRACEFUL_LEAVE_TIMEOUT: Duration = Duration::from_secs(5);

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
    .with_interval(Duration::from_secs(3))
    .with_timeout(Duration::from_secs(6));
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

  /// Directory for RocksDB data.
  #[arg(long)]
  pub db: PathBuf,

  /// Path to persist libp2p identity (protobuf). Default: <db>/node.key
  #[arg(long)]
  pub key: Option<PathBuf>,

  /// Initialize cluster membership on startup.
  ///
  /// Provide all nodes (including self) with multiaddr including /p2p/<peerid>:
  ///   --init --node 12D3KooW...=/ip4/127.0.0.1/tcp/4001/p2p/12D3KooW...
  #[arg(long, default_value_t = false)]
  pub init: bool,

  /// Cluster node addresses in the form: <id>=<multiaddr-with-/p2p/peerid>
  #[arg(long = "node")]
  pub nodes: Vec<String>,

  /// OpenRaft heartbeat interval in milliseconds (leader keepalive cadence).
  #[arg(long, default_value_t = 250)]
  pub raft_keepalive_ms: u64,

  /// OpenRaft election timeout minimum in milliseconds.
  #[arg(long, default_value_t = 299)]
  pub raft_election_timeout_min_ms: u64,

  /// OpenRaft election timeout maximum in milliseconds.
  #[arg(long, default_value_t = 300)]
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

struct NodeIdentity {
  local_peer_id: PeerId,
  node_name: String,
}

struct Libp2pHandles {
  cmd_tx: mpsc::Sender<Command>,
  client: Libp2pClient,
  kv_client: KvClient,
  network: Libp2pNetworkFactory,
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

fn build_libp2p_handles(
  timeout: Duration,
  local_peer_id: PeerId,
) -> (Libp2pHandles, mpsc::Receiver<Command>) {
  let (cmd_tx, cmd_rx) = mpsc::channel(256);
  let client = Libp2pClient::new(cmd_tx.clone(), timeout);
  let kv_client = KvClient::new(cmd_tx.clone(), timeout);
  let sqlite_sync_client = SqliteSyncClient::new(cmd_tx.clone(), timeout);
  let network =
    Libp2pNetworkFactory::new(client.clone(), sqlite_sync_client.clone(), local_peer_id);
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

  let config = openraft::Config {
    heartbeat_interval: opt.raft_keepalive_ms,
    election_timeout_min: opt.raft_election_timeout_min_ms,
    election_timeout_max: opt.raft_election_timeout_max_ms,
    enable_heartbeat: opt.raft_enable_heartbeat,
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
) -> anyhow::Result<libp2p::Swarm<Behaviour>> {
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
      let mut kad = kad::Behaviour::new(peer_id, MemoryStore::new(peer_id));
      kad.set_mode(Some(kad::Mode::Server));
      let gossipsub_config = gossipsub::ConfigBuilder::default()
        .build()
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
            StreamProtocol::new("/openraft/raft/1"),
            ProtocolSupport::Full,
          )],
          cfg.clone(),
        ),
        kv_rpc: request_response::Behaviour::with_codec(
          ProstCodec::<RaftKvRequest, RaftKvResponse>::default(),
          [(StreamProtocol::new("/openraft/kv/1"), ProtocolSupport::Full)],
          cfg.clone(),
        ),
        sqlite_sync_rpc: request_response::Behaviour::with_codec(
          SerdeCodec::<SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage>::default(),
          [(
            StreamProtocol::new("/openraft/sqlite-sync/1"),
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
      cfg.with_idle_connection_timeout(Duration::from_secs(opt.swarm_idle_connection_timeout_secs))
    })
    .build();

  let gossip_topic = gossipsub::IdentTopic::new(GOSSIP_TOPIC);
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

  swarm.listen_on(listen_addr).context("listen_on")?;
  Ok(swarm)
}

fn spawn_libp2p_swarm(
  shutdown: &mut crate::signal::ShutdownHandler,
  cmd_rx: mpsc::Receiver<Command>,
  libp2p: &Libp2pHandles,
) -> tokio::task::JoinHandle<()> {
  let swarm_done = shutdown.push(SERVICE_LIBP2P_SWARM);
  let swarm_shutdown = shutdown.shutdown_rx();
  let network_for_swarm = libp2p.network.clone();
  let dispatcher_for_swarm = Arc::new(OpenRaftDispatcher::new(libp2p.kv_client.clone()));
  let cmd_tx_for_swarm = libp2p.cmd_tx.clone();
  tokio::spawn(async move {
    run_swarm(
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
) -> http::AppState {
  let default_group = default_openraft_group_id();

  http::AppState {
    node_id: opt.id.clone(),
    node_name: identity.node_name.clone(),
    peer_id: identity.local_peer_id.to_string(),
    listen: opt.listen.clone(),
    network: libp2p.network.clone(),
    kv_client: libp2p.kv_client.clone(),
    default_group,
    apalis_email: build_apalis_email_storage(opt.id.clone(), libp2p)
      .expect("apalis group should be configured"),
    sqlite_cache,
  }
}

fn build_apalis_email_storage(
  node_id: NodeId,
  libp2p: &Libp2pHandles,
) -> anyhow::Result<apalis_raft::RaftApalisStorage<apalis_raft::Email>> {
  let group =
    openraft_group(groups::APALIS).ok_or_else(|| anyhow!("apalis raft group is not configured"))?;
  Ok(apalis_raft::build_email_storage(
    node_id,
    groups::APALIS,
    group,
    libp2p.kv_client.clone(),
  ))
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

fn spawn_apalis_worker(
  shutdown: &mut crate::signal::ShutdownHandler,
  worker_name: String,
  storage: apalis_raft::RaftApalisStorage<apalis_raft::Email>,
) -> tokio::task::JoinHandle<()> {
  let apalis_done = shutdown.push(SERVICE_APALIS_WORKER);
  let apalis_shutdown = shutdown.shutdown_rx();
  tokio::spawn(async move {
    let res = apalis_raft::run_email_worker(worker_name, storage, apalis_shutdown).await;
    let _ = apalis_done.send(res);
  })
}

fn spawn_openraft_autoscaler(
  shutdown: &mut crate::signal::ShutdownHandler,
  network: Libp2pNetworkFactory,
) -> tokio::task::JoinHandle<()> {
  let done = shutdown.push(SERVICE_OPENRAFT_AUTOSCALER);
  let shutdown_rx = shutdown.shutdown_rx();
  tokio::spawn(async move {
    run_openraft_autoscaler(network, shutdown_rx).await;
    let _ = done.send(Ok(()));
  })
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

async fn run_openraft_autoscaler(
  network: Libp2pNetworkFactory,
  mut shutdown_rx: crate::signal::ShutdownRx,
) {
  let mut tick = tokio::time::interval(Duration::from_secs(OPENRAFT_AUTOSCALER_INTERVAL_SECS));
  let mut offline_since = BTreeMap::new();
  tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!("shutdown signal received, stopping openraft autoscaler");
        break;
      }
      _ = tick.tick() => {
        let known_nodes = network.known_nodes().await;
        let Some(groups) = openraft_groups().map(|groups| {
          groups
            .iter()
            .map(|(group_id, group)| (group_id.clone(), group.clone()))
            .collect::<Vec<_>>()
        }) else {
          tracing::warn!("openraft groups are not initialized");
          continue;
        };
        for (group_id, group) in &groups {
          if let Err(err) = reconcile_openraft_group(
            group_id,
            group,
            &network,
            &known_nodes,
            &mut offline_since,
          ).await {
            tracing::warn!(group = group_id, error = ?err, "openraft autoscaler reconcile failed");
          }
        }
      }
    }
  }
}

async fn reconcile_openraft_group(
  group_id: &str,
  group: &GroupHandle,
  network: &Libp2pNetworkFactory,
  known_nodes: &[(NodeId, PeerId, Multiaddr)],
  offline_since: &mut BTreeMap<(GroupId, NodeId), tokio::time::Instant>,
) -> anyhow::Result<()> {
  let metrics = group.raft.metrics().borrow_watched().clone();
  if !metrics.state.is_leader() {
    return Ok(());
  }

  let membership = metrics.membership_config.membership();
  let voters = membership.voter_ids().collect::<BTreeSet<_>>();
  let learners = membership.learner_ids().collect::<BTreeSet<_>>();

  if remove_offline_openraft_member_if_expired(group_id, group, &metrics, network, offline_since)
    .await?
  {
    return Ok(());
  }

  if learners.len() < OPENRAFT_MAX_LEARNERS {
    add_next_discovered_learner(group_id, group, known_nodes, &voters, &learners).await?;
  }

  // Refresh metrics after potential learner addition.
  let metrics = group.raft.metrics().borrow_watched().clone();
  let learners_after = metrics
    .membership_config
    .membership()
    .learner_ids()
    .collect::<BTreeSet<_>>();

  // Only pay the cost of counting active voters when there are learners that
  // could potentially be promoted.
  if !learners_after.is_empty() {
    let active_voters = count_active_voter_states(group_id, &metrics, network).await;
    // Promote only when we are strictly below the voter cap so we never
    // exceed OPENRAFT_MAX_VOTERS (the old `<=` comparison was a bug that
    // allowed promotion even when the cap was already reached).
    if active_voters < OPENRAFT_MAX_VOTERS {
      promote_caught_up_learner_if_needed(group_id, group, &metrics, active_voters).await?;
    }
  }

  Ok(())
}

async fn remove_offline_openraft_member_if_expired(
  group_id: &str,
  group: &GroupHandle,
  metrics: &crate::typ::RaftMetrics,
  network: &Libp2pNetworkFactory,
  offline_since: &mut BTreeMap<(GroupId, NodeId), tokio::time::Instant>,
) -> anyhow::Result<bool> {
  let membership = metrics.membership_config.membership();
  let voters = membership.voter_ids().collect::<BTreeSet<_>>();
  let learners = membership.learner_ids().collect::<BTreeSet<_>>();
  let member_ids = membership
    .nodes()
    .map(|(id, _)| id.clone())
    .collect::<BTreeSet<_>>();
  offline_since.retain(|(offline_group, node_id), _| {
    offline_group != group_id || member_ids.contains(node_id)
  });

  let timeout = Duration::from_secs(OPENRAFT_OFFLINE_REMOVE_AFTER_SECS);
  for (node_id, node) in membership.nodes() {
    if *node_id == metrics.id {
      continue;
    }

    let (peer, _) = match parse_p2p_addr(&node.addr) {
      Ok(peer_addr) => peer_addr,
      Err(err) => {
        tracing::warn!(
          group = group_id,
          node_id = %node_id,
          addr = %node.addr,
          error = ?err,
          "skip openraft offline check for node with invalid libp2p address"
        );
        continue;
      }
    };

    let key = (group_id.to_string(), node_id.clone());
    if network.is_peer_connected(&peer).await {
      if offline_since.remove(&key).is_some() {
        tracing::info!(
          group = group_id,
          node_id = %node_id,
          peer = %peer,
          "openraft member reconnected before removal timeout"
        );
      }
      continue;
    }

    let now = tokio::time::Instant::now();
    let Some(since) = offline_since.get(&key).copied() else {
      offline_since.insert(key, now);
      tracing::warn!(
        group = group_id,
        node_id = %node_id,
        peer = %peer,
        timeout_secs = OPENRAFT_OFFLINE_REMOVE_AFTER_SECS,
        "openraft member is disconnected; starting removal timeout"
      );
      continue;
    };

    if now.duration_since(since) < timeout {
      continue;
    }

    let remove_id = node_id.clone();
    let changes = if voters.contains(&remove_id) {
      if voters.len() <= 1 {
        tracing::warn!(
          group = group_id,
          node_id = %remove_id,
          "skip removing the last openraft voter"
        );
        continue;
      }
      ChangeMembers::RemoveVoters(BTreeSet::from([remove_id.clone()]))
    } else if learners.contains(&remove_id) {
      ChangeMembers::RemoveNodes(BTreeSet::from([remove_id.clone()]))
    } else {
      continue;
    };

    tracing::warn!(
      group = group_id,
      node_id = %remove_id,
      peer = %peer,
      timeout_secs = OPENRAFT_OFFLINE_REMOVE_AFTER_SECS,
      "removing disconnected openraft member after timeout"
    );
    group
      .raft
      .change_membership(changes, false)
      .await
      .map_err(|err| {
        anyhow!("remove disconnected node {remove_id} from group {group_id}: {err}")
      })?;
    offline_since.remove(&(group_id.to_string(), remove_id));
    return Ok(true);
  }

  Ok(false)
}

async fn add_next_discovered_learner(
  group_id: &str,
  group: &GroupHandle,
  known_nodes: &[(NodeId, PeerId, Multiaddr)],
  voters: &BTreeSet<NodeId>,
  learners: &BTreeSet<NodeId>,
) -> anyhow::Result<()> {
  for (node_id, _peer, addr) in known_nodes {
    if voters.contains(node_id) || learners.contains(node_id) {
      continue;
    }

    let node = BasicNode {
      addr: addr.to_string(),
    };
    tracing::info!(
      group = group_id,
      node_id = %node_id,
      addr = %addr,
      "adding discovered libp2p node as openraft learner"
    );
    group
      .raft
      .add_learner(node_id.clone(), node, true)
      .await
      .map_err(|err| anyhow!("add learner {node_id} to group {group_id}: {err}"))?;
    break;
  }

  Ok(())
}

/// Returns `true` when the given learner's replication progress is within
/// [`LEARNER_PROMOTE_MAX_LAG`] log entries of the leader, meaning the learner
/// is sufficiently caught-up to be promoted without stalling consensus.
fn learner_is_caught_up(learner_id: &NodeId, metrics: &crate::typ::RaftMetrics) -> bool {
  let Some(replication) = &metrics.replication else {
    // No replication map means we cannot confirm catch-up; be conservative.
    return false;
  };
  let leader_last = metrics.last_log_index.unwrap_or(0);
  let learner_matched = replication
    .get(learner_id)
    .and_then(|log_id| log_id.clone())
    .map(|log_id| log_id.index)
    .unwrap_or(0);
  leader_last.saturating_sub(learner_matched) <= LEARNER_PROMOTE_MAX_LAG
}

/// Promote one caught-up learner to voter when the cluster is under the voter
/// cap. Only learners whose replication lag is within [`LEARNER_PROMOTE_MAX_LAG`]
/// are considered; the rest are skipped until they catch up.
async fn promote_caught_up_learner_if_needed(
  group_id: &str,
  group: &GroupHandle,
  metrics: &crate::typ::RaftMetrics,
  active_voters: usize,
) -> anyhow::Result<()> {
  let membership = metrics.membership_config.membership();
  let learners = membership.learner_ids().collect::<Vec<_>>();

  // Collect caught-up learners and pick a random one.
  let caught_up: Vec<_> = learners
    .iter()
    .filter(|id| learner_is_caught_up(id, metrics))
    .cloned()
    .collect();

  if caught_up.is_empty() {
    if !learners.is_empty() {
      tracing::debug!(
        group = group_id,
        learners = learners.len(),
        "no learner is sufficiently caught up for promotion yet"
      );
    }
    return Ok(());
  }

  let Some(learner_id) = caught_up.choose(&mut rand::rng()).cloned() else {
    return Ok(());
  };

  tracing::info!(
    group = group_id,
    node_id = %learner_id,
    active_voters,
    learners = learners.len(),
    caught_up = caught_up.len(),
    "promoting caught-up openraft learner to voter"
  );
  group
    .raft
    .change_membership(
      ChangeMembers::AddVoterIds(BTreeSet::from([learner_id.clone()])),
      true,
    )
    .await
    .map_err(|err| anyhow!("promote learner {learner_id} in group {group_id}: {err}"))?;

  Ok(())
}

async fn count_active_voter_states(
  group_id: &str,
  local_metrics: &crate::typ::RaftMetrics,
  network: &Libp2pNetworkFactory,
) -> usize {
  let voter_ids = local_metrics
    .membership_config
    .membership()
    .voter_ids()
    .collect::<Vec<_>>();
  let mut count = 0;

  for node_id in voter_ids {
    let state = if node_id == local_metrics.id {
      Some(local_metrics.state)
    } else {
      remote_server_state(group_id, &node_id, network).await
    };

    if matches!(
      state,
      Some(ServerState::Leader | ServerState::Follower | ServerState::Candidate)
    ) {
      count += 1;
    }
  }

  count
}

async fn remote_server_state(
  group_id: &str,
  node_id: &NodeId,
  network: &Libp2pNetworkFactory,
) -> Option<ServerState> {
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
    Ok(RaftRpcResponse::GetMetrics(metrics)) => Some(metrics.state),
    Ok(other) => {
      tracing::debug!(
        group = group_id,
        node_id = %node_id,
        response = ?other,
        "unexpected get-metrics response"
      );
      None
    }
    Err(err) => {
      tracing::debug!(
        group = group_id,
        node_id = %node_id,
        error = ?err,
        "get-metrics failed while counting openraft server states"
      );
      None
    }
  }
}

/// Attempt a best-effort graceful leave for one Raft group.
///
/// If this node is the **leader** it tries to remove itself from the membership
/// so that it transfers leadership before the network drops. This is done with
/// a bounded timeout so that a slow network never blocks shutdown indefinitely.
///
/// If this node is a **follower / learner** or the only voter it simply logs
/// and returns; the autoscaler on other nodes will clean up the stale entry
/// after `OPENRAFT_OFFLINE_REMOVE_AFTER_SECS`.
async fn try_graceful_leave_group(group_id: &str, group: &GroupHandle, self_id: &NodeId) {
  let metrics = group.raft.metrics().borrow_watched().clone();
  let membership = metrics.membership_config.membership();

  if membership.get_node(self_id).is_none() {
    return; // Already not a member.
  }

  if !metrics.state.is_leader() {
    tracing::info!(
      group = group_id,
      state = ?metrics.state,
      "graceful leave skipped: not the leader, autoscaler will handle removal"
    );
    return;
  }

  let voters: BTreeSet<NodeId> = membership.voter_ids().collect();
  let is_voter = voters.contains(self_id);

  if is_voter && voters.len() <= 1 {
    tracing::info!(
      group = group_id,
      "graceful leave skipped: last voter in group"
    );
    return;
  }

  let changes = if is_voter {
    ChangeMembers::RemoveVoters(BTreeSet::from([self_id.clone()]))
  } else {
    ChangeMembers::RemoveNodes(BTreeSet::from([self_id.clone()]))
  };

  tracing::info!(
    group = group_id,
    is_voter,
    "attempting graceful self-removal from raft group before shutdown"
  );

  match tokio::time::timeout(
    GRACEFUL_LEAVE_TIMEOUT,
    group.raft.change_membership(changes, false),
  )
  .await
  {
    Ok(Ok(_)) => tracing::info!(group = group_id, "gracefully left raft group"),
    Ok(Err(err)) => tracing::warn!(group = group_id, error = ?err, "graceful leave failed"),
    Err(_) => tracing::warn!(group = group_id, "graceful leave timed out"),
  }
}

/// Run graceful leave for every configured Raft group.
async fn try_graceful_leave(self_id: NodeId) {
  let Some(groups) = openraft_groups().map(|g| {
    g.iter()
      .map(|(id, group)| (id.clone(), group.clone()))
      .collect::<Vec<_>>()
  }) else {
    return;
  };
  for (group_id, group) in &groups {
    try_graceful_leave_group(group_id, group, &self_id).await;
  }
}

fn spawn_openraft_shutdown(
  shutdown: &mut crate::signal::ShutdownHandler,
  mut shutdown_rx_for_ordering: crate::signal::ShutdownRx,
  self_id: NodeId,
  swarm_handle: tokio::task::JoinHandle<()>,
  http_handle: tokio::task::JoinHandle<()>,
  apalis_handle: tokio::task::JoinHandle<()>,
  autoscaler_handle: tokio::task::JoinHandle<()>,
  sqlite_flusher_handle: Option<tokio::task::JoinHandle<()>>,
) {
  // Openraft should shut down after libp2p swarm has stopped.
  let (openraft_shutdown_tx, mut openraft_shutdown_rx) = crate::signal::channel();
  let raft_done = shutdown.push(SERVICE_OPENRAFT);
  tokio::spawn(async move {
    let _ = openraft_shutdown_rx.changed().await;
    let mut errors = Vec::new();
    if let Some(rafts) = openraft_groups().map(|groups| {
      groups
        .values()
        .map(|group| group.raft.clone())
        .collect::<Vec<_>>()
    }) {
      for raft in rafts {
        if let Err(err) = raft.shutdown().await {
          errors.push(anyhow!("openraft shutdown failed: {err:?}"));
        }
      }
    } else {
      errors.push(anyhow!("openraft groups are not initialized"));
    }
    let res = match errors.len() {
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
    };
    let _ = raft_done.send(res);
  });

  tokio::spawn(async move {
    let _ = shutdown_rx_for_ordering.changed().await;
    // Attempt graceful self-removal while the network is still up so that
    // other nodes do not have to wait for the offline-timeout to expire.
    try_graceful_leave(self_id).await;
    let _ = swarm_handle.await;
    let _ = http_handle.await;
    let _ = apalis_handle.await;
    let _ = autoscaler_handle.await;
    if let Some(sqlite_flusher_handle) = sqlite_flusher_handle {
      let _ = sqlite_flusher_handle.await;
    }
    let _ = openraft_shutdown_tx.send(());
  });
}

async fn register_members(
  network: &Libp2pNetworkFactory,
  nodes: &[String],
) -> anyhow::Result<BTreeMap<NodeId, BasicNode>> {
  let mut members: BTreeMap<NodeId, BasicNode> = BTreeMap::new();
  for n in nodes {
    let (id, addr) = parse_node_kv(n)?;
    network.register_node(id.clone(), &addr).await?;
    members.insert(
      id,
      BasicNode {
        addr: addr.to_string(),
      },
    );
  }
  Ok(members)
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

async fn cleanup_removed_local_groups(
  db_dir: &Path,
  self_id: &NodeId,
  group_ids: &[GroupId],
  network: &Libp2pNetworkFactory,
) -> anyhow::Result<()> {
  for group_id in group_ids {
    let Some(local_membership) = store::read_persisted_membership_for_group(db_dir, group_id)?
    else {
      continue;
    };

    if local_membership.membership().get_node(self_id).is_none() {
      continue;
    }

    let Some(remote_metrics) = fetch_remote_group_metrics(group_id, self_id, network).await else {
      tracing::debug!(
        group = group_id,
        node_id = %self_id,
        "skip local data cleanup because no remote openraft metrics are available"
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
  }

  Ok(())
}

async fn fetch_remote_group_metrics(
  group_id: &str,
  self_id: &NodeId,
  network: &Libp2pNetworkFactory,
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
      Ok(RaftRpcResponse::GetMetrics(metrics)) => return Some(metrics),
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

async fn maybe_bootstrap(
  client: &Libp2pClient,
  members: &BTreeMap<NodeId, BasicNode>,
  self_id: NodeId,
) {
  let Ok(bootstrap_name) = env::var(ENV_BOOTSTRAP_NAME) else {
    return;
  };

  let mut bootstrap_target: Option<(NodeId, String)> = None;
  for (id, node) in members {
    if node_name_for_id(id) == bootstrap_name {
      bootstrap_target = Some((id.clone(), node.addr.clone()));
      break;
    }
  }

  match bootstrap_target {
    Some((id, addr)) if id == self_id => {
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

async fn maybe_init_cluster(
  members: BTreeMap<NodeId, BasicNode>,
  self_id: NodeId,
  init: bool,
) -> anyhow::Result<()> {
  if !init {
    return Ok(());
  }

  if !members.contains_key(&self_id) {
    return Err(anyhow!("--init requires providing self in --node list"));
  }
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
    .unwrap_or_else(|| "default".to_string())
}

async fn await_shutdown(shutdown: crate::signal::ShutdownHandler) -> anyhow::Result<()> {
  let (_tx, _rx, results) = shutdown.await_any_then_shutdown().await;
  let mut errors = Vec::new();
  for (service, res) in results {
    if let Err(err) = res {
      tracing::error!(service, error = ?err, "shutdown task failed");
      errors.push((service, err));
    }
  }

  if errors.is_empty() {
    tracing::info!("shutdown complete");
    return Ok(());
  }

  if errors.len() == 1 {
    let (service, err) = errors.pop().unwrap();
    return Err(anyhow!("shutdown error in {service}: {err}"));
  }

  let mut msg = String::new();
  use std::fmt::Write as _;
  let _ = writeln!(&mut msg, "encountered {} shutdown errors:", errors.len());
  for (service, err) in errors {
    let _ = writeln!(&mut msg, "  {service}: {err}");
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
  let swarm = build_swarm(&opt, listen_addr, local_key)?;
  let swarm = Arc::new(tokio::sync::Mutex::new(swarm));
  set_libp2p_swarm(swarm).map_err(|_| anyhow!("global libp2p swarm already initialized"))?;
  let mut shutdown = crate::signal::spawn_handler();
  let shutdown_rx_for_ordering = shutdown.shutdown_rx();

  let swarm_handle = spawn_libp2p_swarm(&mut shutdown, cmd_rx, &libp2p);

  let members = register_members(&libp2p.network, &opt.nodes).await?;

  // Dials issued by register_members are fire-and-forget; wait a short window
  // for at least one peer connection to be established before running the
  // removed-node check, so the check can actually reach remote nodes.
  if !opt.nodes.is_empty() {
    let peer_connected =
      wait_for_any_peer_connected(&libp2p.network, STARTUP_PEER_CONNECT_WAIT).await;
    if !peer_connected {
      tracing::warn!(
        "no peer connected within {:?}; removed-node detection may be skipped",
        STARTUP_PEER_CONNECT_WAIT
      );
    }
  }

  cleanup_removed_local_groups(&opt.db, &opt.id, &group_ids, &libp2p.network).await?;

  let openraft_groups = start_openraft_groups(
    &opt,
    opt.id.clone(),
    &opt.db,
    libp2p.network.clone(),
    &group_ids,
  )
  .await?;
  set_openraft_groups(openraft_groups)
    .map_err(|_| anyhow!("global openraft groups already initialized"))?;

  let sqlite_cache = if opt.disable_sqlite_cache {
    None
  } else {
    Some(SqliteCache::connect_in_db_dir(&opt.db, &opt.redis_url).await?)
  };
  if let Some(cache) = sqlite_cache.clone() {
    sqlite_cache::set_sqlite_cache(cache)
      .map_err(|_| anyhow!("global sqlite cache already initialized"))?;
  }
  let sqlite_flush_group_id = default_openraft_group_id();

  let http_state = build_http_state(&opt, &identity, &libp2p, sqlite_cache.clone());
  let apalis_storage = http_state.apalis_email.clone();
  let apalis_worker_name = format!("raft-email-worker-{}", opt.id);
  let http_handle = spawn_http(&mut shutdown, http_addr, http_state);
  let apalis_handle = spawn_apalis_worker(&mut shutdown, apalis_worker_name, apalis_storage);

  maybe_bootstrap(&libp2p.client, &members, opt.id.clone()).await;
  maybe_init_cluster(members, opt.id.clone(), opt.init).await?;

  let autoscaler_handle = spawn_openraft_autoscaler(&mut shutdown, libp2p.network.clone());
  let sqlite_flusher_handle = sqlite_cache.map(|_| {
    spawn_sqlite_cache_flusher(
      &mut shutdown,
      opt.id.clone(),
      sqlite_flush_group_id,
      libp2p.network.clone(),
      libp2p.kv_client.clone(),
    )
  });

  spawn_openraft_shutdown(
    &mut shutdown,
    shutdown_rx_for_ordering,
    opt.id.clone(),
    swarm_handle,
    http_handle,
    apalis_handle,
    autoscaler_handle,
    sqlite_flusher_handle,
  );

  await_shutdown(shutdown).await
}

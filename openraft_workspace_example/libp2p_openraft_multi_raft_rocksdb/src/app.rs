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
use kameo::remote;
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
    SERVICE_OPENRAFT_AUTOSCALER,
  },
  groups, http,
  kameo_remote::KameoState,
  network::{
    openraft_dispatcher::OpenRaftDispatcher,
    proto_codec::{ProstCodec, ProtoCodec},
    raft_bridge::P2PNetworkFactoryWrapper,
    rpc::{RaftRpcOp, RaftRpcRequest, RaftRpcResponse},
    swarm::{Behaviour, Command, GOSSIP_TOPIC, KvClient, Libp2pClient, run_swarm},
    transport::{Libp2pNetworkFactory, parse_p2p_addr},
  },
  proto::raft_kv::{RaftKvRequest, RaftKvResponse},
  store,
  typ::Raft,
};

const ENV_SELF_NAME: &str = "LIBP2P_SELF_NAME";
const ENV_BOOTSTRAP_NAME: &str = "LIBP2P_BOOTSTRAP_NAME";
const OPENRAFT_MAX_LEARNERS: usize = 5;
const OPENRAFT_MAX_VOTERS: usize = 5;
const OPENRAFT_AUTOSCALER_INTERVAL_SECS: u64 = 5;

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

  /// Run the kameo remote demo instead of the raft node.
  #[arg(long, default_value_t = false)]
  pub kameo_remote: bool,

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

struct OpenraftHandles {
  groups: GroupHandleMap,
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
  let network = Libp2pNetworkFactory::new(client.clone(), local_peer_id);
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
) -> anyhow::Result<OpenraftHandles> {
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

  Ok(OpenraftHandles { groups })
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
      let kameo = remote::Behaviour::new(
        peer_id,
        remote::messaging::Config::default()
          .with_request_timeout(Duration::from_secs(30))
          .with_max_concurrent_streams(100),
      );

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
          cfg,
        ),
        gossipsub,
        ping,
        mdns,
        kad,
        kameo,
      })
    })
    .context("build behaviour")?
    .with_swarm_config(|cfg| {
      cfg.with_idle_connection_timeout(Duration::from_secs(opt.swarm_idle_connection_timeout_secs))
    })
    .build();

  swarm.behaviour_mut().kameo.init_global();

  let gossip_topic = gossipsub::IdentTopic::new(GOSSIP_TOPIC);
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
  swarm: libp2p::Swarm<Behaviour>,
  cmd_rx: mpsc::Receiver<Command>,
  libp2p: &Libp2pHandles,
  openraft: &OpenraftHandles,
) -> tokio::task::JoinHandle<()> {
  let swarm_done = shutdown.push(SERVICE_LIBP2P_SWARM);
  let swarm_shutdown = shutdown.shutdown_rx();
  let network_for_swarm = libp2p.network.clone();
  let dispatcher_for_swarm = Arc::new(OpenRaftDispatcher::new(
    openraft.groups.clone(),
    libp2p.kv_client.clone(),
  ));
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
  openraft: &OpenraftHandles,
  kameo_state: Arc<KameoState>,
) -> http::AppState {
  let default_group = if openraft.groups.contains_key(groups::USERS) {
    groups::USERS.to_string()
  } else {
    openraft
      .groups
      .keys()
      .next()
      .cloned()
      .unwrap_or_else(|| "default".to_string())
  };

  http::AppState {
    node_id: opt.id.clone(),
    node_name: identity.node_name.clone(),
    peer_id: identity.local_peer_id.to_string(),
    listen: opt.listen.clone(),
    network: libp2p.network.clone(),
    groups: openraft.groups.clone(),
    kv_client: libp2p.kv_client.clone(),
    default_group,
    apalis_email: build_apalis_email_storage(opt.id.clone(), libp2p, openraft)
      .expect("apalis group should be configured"),
    kameo: kameo_state,
  }
}

fn build_apalis_email_storage(
  node_id: NodeId,
  libp2p: &Libp2pHandles,
  openraft: &OpenraftHandles,
) -> anyhow::Result<apalis_raft::RaftApalisStorage<apalis_raft::Email>> {
  let group = openraft
    .groups
    .get(groups::APALIS)
    .cloned()
    .ok_or_else(|| anyhow!("apalis raft group is not configured"))?;
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
  groups: GroupHandleMap,
  network: Libp2pNetworkFactory,
) -> tokio::task::JoinHandle<()> {
  let done = shutdown.push(SERVICE_OPENRAFT_AUTOSCALER);
  let shutdown_rx = shutdown.shutdown_rx();
  tokio::spawn(async move {
    run_openraft_autoscaler(groups, network, shutdown_rx).await;
    let _ = done.send(Ok(()));
  })
}

async fn run_openraft_autoscaler(
  groups: GroupHandleMap,
  network: Libp2pNetworkFactory,
  mut shutdown_rx: crate::signal::ShutdownRx,
) {
  let mut tick = tokio::time::interval(Duration::from_secs(OPENRAFT_AUTOSCALER_INTERVAL_SECS));
  tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!("shutdown signal received, stopping openraft autoscaler");
        break;
      }
      _ = tick.tick() => {
        let known_nodes = network.known_nodes().await;
        for (group_id, group) in &groups {
          if let Err(err) = reconcile_openraft_group(group_id, group, &network, &known_nodes).await {
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
) -> anyhow::Result<()> {
  let metrics = group.raft.metrics().borrow_watched().clone();
  if !metrics.state.is_leader() {
    return Ok(());
  }

  let membership = metrics.membership_config.membership();
  let voters = membership.voter_ids().collect::<BTreeSet<_>>();
  let learners = membership.learner_ids().collect::<BTreeSet<_>>();

  if learners.len() < OPENRAFT_MAX_LEARNERS {
    add_next_discovered_learner(group_id, group, known_nodes, &voters, &learners).await?;
  }

  let metrics = group.raft.metrics().borrow_watched().clone();
  if metrics.state.is_leader() {
    let active_voters = count_active_voter_states(group_id, &metrics, network).await;
    if active_voters <= OPENRAFT_MAX_VOTERS {
      promote_random_learner_if_needed(group_id, group, &metrics, active_voters).await?;
    }
  }

  Ok(())
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

async fn promote_random_learner_if_needed(
  group_id: &str,
  group: &GroupHandle,
  metrics: &crate::typ::RaftMetrics,
  active_voters: usize,
) -> anyhow::Result<()> {
  let membership = metrics.membership_config.membership();
  let learners = membership.learner_ids().collect::<Vec<_>>();
  let Some(learner_id) = learners.choose(&mut rand::rng()).cloned() else {
    return Ok(());
  };

  tracing::info!(
    group = group_id,
    node_id = %learner_id,
    active_voters,
    learners = learners.len(),
    "promoting openraft learner to voter"
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

fn spawn_openraft_shutdown(
  shutdown: &mut crate::signal::ShutdownHandler,
  rafts: Vec<Raft>,
  mut shutdown_rx_for_ordering: crate::signal::ShutdownRx,
  swarm_handle: tokio::task::JoinHandle<()>,
  http_handle: tokio::task::JoinHandle<()>,
  apalis_handle: tokio::task::JoinHandle<()>,
  autoscaler_handle: tokio::task::JoinHandle<()>,
) {
  // Openraft should shut down after libp2p swarm has stopped.
  let (openraft_shutdown_tx, mut openraft_shutdown_rx) = crate::signal::channel();
  let raft_done = shutdown.push(SERVICE_OPENRAFT);
  tokio::spawn(async move {
    let _ = openraft_shutdown_rx.changed().await;
    let mut errors = Vec::new();
    for raft in rafts {
      if let Err(err) = raft.shutdown().await {
        errors.push(anyhow!("openraft shutdown failed: {err:?}"));
      }
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
    let _ = swarm_handle.await;
    let _ = http_handle.await;
    let _ = apalis_handle.await;
    let _ = autoscaler_handle.await;
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
  groups: &GroupHandleMap,
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
  tracing::info!(
    "initializing cluster membership: {} nodes, {} groups",
    members.len(),
    groups.len()
  );
  for (group_id, group) in groups {
    let res = group.raft.initialize(members.clone()).await;
    tracing::info!(group = group_id, "initialize result: {:?}", res);
  }
  Ok(())
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
  let openraft = start_openraft_groups(
    &opt,
    opt.id.clone(),
    &opt.db,
    libp2p.network.clone(),
    &group_ids,
  )
  .await?;

  let swarm = build_swarm(&opt, listen_addr, local_key)?;
  let mut shutdown = crate::signal::spawn_handler();
  let shutdown_rx_for_ordering = shutdown.shutdown_rx();

  let swarm_handle = spawn_libp2p_swarm(&mut shutdown, swarm, cmd_rx, &libp2p, &openraft);

  if opt.kameo_remote {
    let custom_swarm = env::var("CUSTOM_SWARM").is_ok();
    return crate::kameo_remote::run(custom_swarm, http_addr).await;
  }

  let kameo_state =
    crate::kameo_remote::register_incrementor(identity.local_peer_id.clone()).await?;
  let http_state = build_http_state(&opt, &identity, &libp2p, &openraft, kameo_state);
  let apalis_storage = http_state.apalis_email.clone();
  let apalis_worker_name = format!("raft-email-worker-{}", opt.id);
  let http_handle = spawn_http(&mut shutdown, http_addr, http_state);
  let apalis_handle = spawn_apalis_worker(&mut shutdown, apalis_worker_name, apalis_storage);

  let members = register_members(&libp2p.network, &opt.nodes).await?;
  maybe_bootstrap(&libp2p.client, &members, opt.id.clone()).await;
  maybe_init_cluster(&openraft.groups, members, opt.id.clone(), opt.init).await?;

  let autoscaler_handle = spawn_openraft_autoscaler(
    &mut shutdown,
    openraft.groups.clone(),
    libp2p.network.clone(),
  );

  spawn_openraft_shutdown(
    &mut shutdown,
    openraft
      .groups
      .values()
      .map(|group| group.raft.clone())
      .collect(),
    shutdown_rx_for_ordering,
    swarm_handle,
    http_handle,
    apalis_handle,
    autoscaler_handle,
  );

  await_shutdown(shutdown).await
}

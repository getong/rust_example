//! Node process entry point: CLI options, run() orchestration, and shared
//! glue (HTTP/state spawning, graceful shutdown). The startup, control,
//! worker, membership-join, and maintenance concerns live in the sibling
//! submodules; everything is re-exported here so `crate::app::X` paths keep
//! working.

use std::{
  env,
  net::SocketAddr,
  path::{Path, PathBuf},
  time::Duration,
};

use anyhow::{Context, anyhow};
use clap::{ArgAction, Parser};
use futures::{AsyncRead, AsyncWrite};
use libp2p::{
  Multiaddr, PeerId, Transport, identity,
  kad::{self},
  ping, websocket,
};

use crate::{
  GroupId, NodeId,
  constants::SERVICE_HTTP,
  groups, http,
  network::{
    swarm::{CommandSenders, KvClient, Libp2pClient, OPENRAFT_CLUSTER_PROVIDER_KEY},
    transport::{Libp2pNetworkFactory, parse_p2p_addr},
  },
  sqlite_cache::SqliteCache,
};

mod control_services;
mod membership_join;
mod node_maintenance;
mod startup;
mod worker_services;

pub(crate) use control_services::*;
pub(crate) use membership_join::*;
pub use node_maintenance::*;
pub(crate) use startup::*;
pub(crate) use worker_services::*;

pub(crate) const ENV_SELF_NAME: &str = "LIBP2P_SELF_NAME";
pub(crate) const OPENRAFT_LEADER_CONTROLLER_INTERVAL_SECS: u64 = 1;
pub(crate) const MEMBERSHIP_GUARD_TICK_SECS: u64 = 5;
pub(crate) const EVICTED_LEARNER_REGISTER_RETRY_SECS: u64 = 10;
pub(crate) const SQLITE_CACHE_FLUSH_INTERVAL_SECS: u64 = 5;
pub(crate) const CONTROL_PROMOTION_POLL_INTERVAL_SECS: u64 = 2;
/// Poll cadence of the control demotion watcher (kad Server → Client when
/// this node is evicted from the voter set while still running).
pub(crate) const CONTROL_DEMOTION_POLL_INTERVAL_SECS: u64 = 30;
/// Consecutive confirmed "not a voter anywhere" rounds required before the
/// node demotes its kademlia role. Guards against transient membership
/// views during joint-consensus changes.
pub(crate) const CONTROL_DEMOTION_CONFIRMATIONS: u32 = 3;
pub(crate) const CONTROL_JOIN_POLL_INTERVAL_SECS: u64 = 2;
pub(crate) const CONTROL_JOIN_CATCH_UP_TIMEOUT_SECS: u64 = 30;
pub const OPENRAFT_KAD_PROVIDER_RECORD_TTL: Duration = Duration::from_secs(180);
pub const OPENRAFT_KAD_PROVIDER_PUBLICATION_INTERVAL: Duration = Duration::from_secs(60);
/// Kademlia's built-in periodic bootstrap cadence. Bootstraps also trigger
/// automatically (throttled) whenever a new peer enters the routing table,
/// so no manual bootstrap calls are needed anywhere.
pub const OPENRAFT_KAD_PERIODIC_BOOTSTRAP_INTERVAL: Duration = Duration::from_secs(60);
pub const PING_INTERVAL: Duration = Duration::from_secs(3);
pub const PING_TIMEOUT: Duration = Duration::from_secs(6);
pub(crate) const DEFAULT_MAX_CONTROL_NODES: usize = 3;
/// How long to wait for at least one remote peer to become connected before
/// running the startup "was this node removed?" membership check. Without
/// this wait the check always sees zero connected peers and silently skips,
/// missing the case where the node was evicted while it was offline.
pub(crate) const STARTUP_PEER_CONNECT_WAIT: Duration = Duration::from_secs(8);
/// Poll cadence of the post-startup openraft verification task.
pub(crate) const STARTUP_VERIFY_POLL_INTERVAL: Duration = Duration::from_secs(3);
/// How often the verification task warns while a group still has no leader.
pub(crate) const STARTUP_NO_LEADER_WARN_INTERVAL: Duration = Duration::from_secs(15);
/// Poll cadence of the known-nodes address book pruner.
pub(crate) const KNOWN_NODE_PRUNE_POLL_INTERVAL: Duration = Duration::from_secs(5);
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

/// Load `.env` (working directory first, then the crate manifest directory)
/// via `dotenvy`, which never overrides variables already present in the
/// environment. Replaces a hand-written parser that called the
/// (Rust-1.66+-unsafe) `env::set_var` directly; the load still happens at
/// the very top of `run()` before any worker threads read the environment.
pub(crate) fn load_env_file() {
  let candidates = [
    PathBuf::from(".env"),
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".env"),
  ];

  for path in candidates {
    match dotenvy::from_path(&path) {
      Ok(()) => return,
      Err(err) if err.not_found() => continue,
      Err(err) => {
        tracing::warn!(path = %path.display(), error = %err, "failed to load .env file");
        return;
      }
    }
  }
}

pub(crate) fn node_name_for_id(id: &NodeId) -> String {
  let key = format!("LIBP2P_NODE_NAME_{id}");
  env::var(key).unwrap_or_else(|_| format!("node-{id}"))
}

#[derive(Clone)]
pub(crate) struct NodeIdentity {
  pub(crate) local_peer_id: PeerId,
  pub(crate) node_name: String,
}

#[derive(Clone)]
pub(crate) struct Libp2pHandles {
  pub(crate) cmd_tx: CommandSenders,
  pub(crate) client: Libp2pClient,
  pub(crate) kv_client: KvClient,
  pub(crate) network: Libp2pNetworkFactory,
}

#[derive(Clone)]
pub(crate) struct ControlRuntime {
  pub(crate) opt: Opt,
  pub(crate) identity: NodeIdentity,
  pub(crate) libp2p: Libp2pHandles,
  pub(crate) group_ids: Vec<GroupId>,
  /// Raft group registry, resolved once at the composition root (`run()`)
  /// and injected everywhere below instead of reaching for the global.
  pub(crate) registry: crate::GroupRegistry,
}

pub(crate) enum StartupMode {
  Worker { known_control_nodes: Vec<NodeId> },
  Control,
}

pub(crate) fn build_http_state(
  opt: &Opt,
  identity: &NodeIdentity,
  libp2p: &Libp2pHandles,
  sqlite_cache: Option<SqliteCache>,
  task_frontend: http::TaskFrontend,
) -> http::AppState {
  let registry = crate::global_group_registry().clone();
  let default_group = default_openraft_group_id(&registry);

  http::AppState {
    registry,
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

pub(crate) fn spawn_http(
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

pub(crate) fn linked_shutdown(
  parent_rx: crate::signal::ShutdownRx,
) -> crate::signal::ShutdownHandler {
  let (tx, rx) = crate::signal::channel();
  let tx_for_parent = tx.clone();
  let mut parent_rx = parent_rx;
  tokio::spawn(async move {
    let _ = parent_rx.changed().await;
    let _ = tx_for_parent.send(());
  });
  crate::signal::ShutdownHandler::new(tx, rx)
}

pub(crate) fn default_openraft_group_id(registry: &crate::GroupRegistry) -> GroupId {
  if registry
    .all()
    .is_some_and(|raft_groups| raft_groups.contains_key(groups::USERS))
  {
    return groups::USERS.to_string();
  }

  registry
    .all()
    .and_then(|raft_groups| raft_groups.keys().next().cloned())
    .unwrap_or_else(|| groups::USERS.to_string())
}

pub(crate) fn collect_shutdown_errors(
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

pub(crate) async fn shutdown_openraft_groups(
  registry: &crate::GroupRegistry,
) -> anyhow::Result<()> {
  let Some(rafts) = registry.all().map(|groups| {
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

pub(crate) async fn run_graceful_shutdown(
  registry: &crate::GroupRegistry,
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

  if let Err(err) = shutdown_openraft_groups(registry).await {
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

pub(crate) fn combine_errors(label: &str, mut errors: Vec<anyhow::Error>) -> anyhow::Result<()> {
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
  // Seed the hot-reloadable runtime config from the CLI; POST /config can
  // change these values later without a restart.
  crate::runtime_config::install(crate::runtime_config::RuntimeConfig {
    node_announce_interval_secs: NODE_ANNOUNCE_INTERVAL.as_secs(),
    voter_replace_timeout_secs: opt.voter_replace_timeout_secs,
  });
  let http_addr: SocketAddr = opt.http.parse().context("invalid --http")?;

  std::fs::create_dir_all(&opt.db).context("create db dir")?;

  let (local_key, identity) = init_node_identity(&opt)?;
  let listen_addr = parse_listen_addr(&opt)?;

  let timeout = Duration::from_secs(5);
  let (libp2p, cmd_rx_high, cmd_rx_low) =
    build_libp2p_handles(timeout, identity.local_peer_id.clone());

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

  let swarm_handle = spawn_libp2p_swarm(
    swarm,
    &mut libp2p_shutdown,
    cmd_rx_high,
    cmd_rx_low,
    &libp2p,
  );

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
  // Composition root: resolve the process-wide registry once; everything
  // below receives it as a parameter.
  let registry = crate::global_group_registry().clone();
  registry.set(group_handles);
  maybe_initialize_bootstrap_openraft(
    &registry,
    opt.id.clone(),
    advertise_addr.clone(),
    bootstrap_self,
  )
  .await?;

  // Verify that every openraft group actually becomes operational after this
  // (possibly out-of-order) startup, and self-correct evicted control nodes.
  tokio::spawn(run_openraft_startup_verifier(
    registry.clone(),
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
      registry.clone(),
      opt.id.clone(),
      libp2p.network.clone(),
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
    registry: registry.clone(),
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
          all_control_nodes.push(NodeId::from(peer));
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

  let graceful_shutdown_result = run_graceful_shutdown(
    &registry,
    &libp2p_client_for_shutdown,
    libp2p_shutdown,
    swarm_handle,
  )
  .await;

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

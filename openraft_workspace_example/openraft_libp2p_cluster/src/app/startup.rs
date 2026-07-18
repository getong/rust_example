//! Boot-time construction and startup-mode decisions: node identity, swarm
//! and libp2p handle wiring, raft group creation, configured-member
//! registration, removed-node cleanup, bootstrap init, and remote metrics
//! probes shared by the startup checks.

use std::{collections::BTreeMap, env, path::Path, sync::Arc, time::Duration};

use anyhow::{Context, anyhow};
use libp2p::{
  Multiaddr, PeerId, StreamProtocol, Swarm, Transport,
  core::upgrade::Version,
  gossipsub, identity,
  kad::{self, store::MemoryStore},
  mdns, noise,
  request_response::{self, ProtocolSupport},
  tcp, tls, websocket, yamux,
};
use openraft::BasicNode;
use tokio::sync::mpsc;

use super::*;
use crate::{
  GroupHandle, GroupHandleMap, GroupId, NodeId,
  constants::SERVICE_LIBP2P_SWARM,
  network::{
    openraft_dispatcher::OpenRaftDispatcher,
    openraft_sync::{OPENRAFT_SYNC_AVAILABLE_TOPIC, OPENRAFT_SYNC_TOPIC},
    proto_codec::UnifiedCodec,
    raft_bridge::P2PNetworkFactoryWrapper,
    rpc::{RaftRpcOp, RaftRpcRequest, RaftRpcResponse},
    swarm::{
      Behaviour, Command, CommandSenders, GOSSIP_TOPIC, KvClient, Libp2pClient,
      NODE_ANNOUNCE_TOPIC, OPENRAFT_CLUSTER_PROVIDER_KEY, SqliteSyncClient, SwarmContext,
      TaskRpcClient, WASM_MODULES_TOPIC, WasmSyncClient, run_swarm,
    },
    transport::{Libp2pNetworkFactory, parse_p2p_addr},
  },
  store,
  typ::Raft,
};

pub(crate) fn is_bootstrap_node(bootstrap_nodes: &[(NodeId, String)], self_id: &NodeId) -> bool {
  bootstrap_nodes.iter().any(|(id, _addr)| id == self_id)
}

pub(crate) fn init_node_identity(opt: &Opt) -> anyhow::Result<(identity::Keypair, NodeIdentity)> {
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

pub(crate) fn parse_listen_addr(opt: &Opt) -> anyhow::Result<Multiaddr> {
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

pub(crate) fn local_advertise_addr(opt: &Opt) -> anyhow::Result<String> {
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

pub(crate) fn build_libp2p_handles(
  timeout: Duration,
  local_peer_id: PeerId,
) -> (
  Libp2pHandles,
  mpsc::Receiver<Command>,
  mpsc::Receiver<Command>,
) {
  // Two priority classes into the swarm loop: raft commands (Libp2pClient)
  // ride the high channel, everything else (kv / sqlite-sync / task) rides
  // low, so a KV burst cannot delay raft heartbeats or votes.
  //
  // Capacity 256 matches the outbound RPC guard's global in-flight cap
  // (`RPC_GLOBAL_MAX_CONCURRENT`): the guard admits at most 256 concurrent
  // RPCs, so a full channel means the loop is genuinely behind, and the
  // resulting send backpressure is the intended signal.
  let (cmd_tx_high, cmd_rx_high) = mpsc::channel(256);
  let (cmd_tx_low, cmd_rx_low) = mpsc::channel(256);
  let client = Libp2pClient::new(cmd_tx_high.clone(), timeout);
  let kv_client = KvClient::new(cmd_tx_low.clone(), timeout);
  let sqlite_sync_client = SqliteSyncClient::new(cmd_tx_low.clone(), timeout);
  let task_rpc_client = TaskRpcClient::new(cmd_tx_low.clone(), timeout);
  let wasm_sync_client = WasmSyncClient::new(cmd_tx_low.clone(), timeout);
  let network = Libp2pNetworkFactory::new(
    client.clone(),
    kv_client.clone(),
    sqlite_sync_client.clone(),
    task_rpc_client,
    wasm_sync_client,
    local_peer_id,
  );
  (
    Libp2pHandles {
      cmd_tx: CommandSenders {
        high: cmd_tx_high,
        low: cmd_tx_low,
      },
      client,
      kv_client,
      network,
    },
    cmd_rx_high,
    cmd_rx_low,
  )
}

pub(crate) async fn start_openraft_groups(
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
    // Elections start disabled and are switched on after a per-node random
    // pre-election delay (see below), so a cluster-wide restart or partition
    // recovery does not have every node campaigning at the same instant.
    enable_elect: false,
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

    spawn_enable_elect_after_delay(&raft, &node_id, group_id, opt.raft_election_timeout_max_ms);

    groups.insert(group_id.clone(), GroupHandle::new(raft, kv_data));
  }

  Ok(groups)
}

/// Enable elections for `raft` after a pre-election delay unique to this
/// (node, group) pair. When many nodes start (or a partition heals) at the
/// same moment, they would otherwise all notice the missing leader within
/// one election window and campaign concurrently; openraft's election
/// timeout randomization alone spreads candidacies over at most
/// `election_timeout_max - election_timeout_min`, which is not enough for a
/// large cluster. The delay is hash-derived — deterministic per node, no
/// randomness dependency — and spans `0..2 * election_timeout_max`, i.e. a
/// few hundred ms to seconds, so it never meaningfully delays recovery.
fn spawn_enable_elect_after_delay(
  raft: &Raft,
  node_id: &NodeId,
  group_id: &GroupId,
  election_timeout_max_ms: u64,
) {
  use std::hash::{Hash, Hasher};

  let mut hasher = std::collections::hash_map::DefaultHasher::new();
  node_id.hash(&mut hasher);
  group_id.hash(&mut hasher);
  let window_ms = election_timeout_max_ms.saturating_mul(2).max(1);
  let delay = Duration::from_millis(hasher.finish() % window_ms);

  let raft = raft.clone();
  let group_id = group_id.clone();
  tokio::spawn(async move {
    tokio::time::sleep(delay).await;
    raft.runtime_config().elect(true);
    tracing::info!(
      group = %group_id,
      delay_ms = delay.as_millis(),
      "pre-election delay elapsed; elections enabled"
    );
  });
}

pub(crate) fn build_swarm(
  opt: &Opt,
  listen_addr: Multiaddr,
  local_key: identity::Keypair,
) -> anyhow::Result<Swarm<Behaviour>> {
  // Multiplexing (3.1): every transport multiplexes many concurrent RPC
  // streams over one connection, so raft replication fan-out, snapshot
  // transfer, and KV traffic to the same peer never serialize behind each
  // other or pay per-request connection setup:
  //   - QUIC has native stream multiplexing (one stream per request-response exchange, no
  //     head-of-line blocking between streams);
  //   - TCP/WebSocket use yamux, whose default `max_num_streams` (512 per connection) matches
  //     `RPC_MAX_CONCURRENT_STREAMS` below.
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
        let dns_transport =
          build_dns_transport(|| tcp::tokio::Transport::new(tcp::Config::default()));
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
      // Raise the stream cap above the RPC guards' concurrency budget and
      // stretch the substream timeout to cover snapshot transfers; see the
      // constants' docs for the sizing rationale.
      let cfg = request_response::Config::default()
        .with_request_timeout(crate::network::swarm::RPC_PROTOCOL_REQUEST_TIMEOUT)
        .with_max_concurrent_streams(crate::network::swarm::RPC_MAX_CONCURRENT_STREAMS);
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
        rpc: request_response::Behaviour::with_codec(
          UnifiedCodec::default(),
          [(
            StreamProtocol::new(crate::network::swarm::UNIFIED_RPC_PROTOCOL),
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
  let sync_available_topic = gossipsub::IdentTopic::new(OPENRAFT_SYNC_AVAILABLE_TOPIC);
  swarm
    .behaviour_mut()
    .gossipsub
    .subscribe(&sync_available_topic)
    .context("openraft snapshot-available gossipsub subscribe")?;
  let wasm_modules_topic = gossipsub::IdentTopic::new(WASM_MODULES_TOPIC);
  swarm
    .behaviour_mut()
    .gossipsub
    .subscribe(&wasm_modules_topic)
    .context("wasm modules gossipsub subscribe")?;

  swarm.listen_on(listen_addr).context("listen_on")?;
  Ok(swarm)
}

pub(crate) fn spawn_libp2p_swarm(
  swarm: Swarm<Behaviour>,
  shutdown: &mut crate::signal::ShutdownHandler,
  cmd_rx_high: mpsc::Receiver<Command>,
  cmd_rx_low: mpsc::Receiver<Command>,
  libp2p: &Libp2pHandles,
  registry: crate::GroupRegistry,
) -> tokio::task::JoinHandle<()> {
  let swarm_done = shutdown.push(SERVICE_LIBP2P_SWARM);
  let ctx = SwarmContext {
    swarm,
    cmd_rx_high,
    cmd_rx_low,
    cmd_tx: libp2p.cmd_tx.clone(),
    network: libp2p.network.clone(),
    dispatcher: Arc::new(OpenRaftDispatcher::with_registry(registry.clone())),
    registry,
    shutdown_rx: shutdown.shutdown_rx(),
  };
  tokio::spawn(async move {
    run_swarm(ctx).await;
    let _ = swarm_done.send(Ok(()));
  })
}

pub(crate) async fn register_members(
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

pub(crate) fn configured_nodes(opt: &Opt) -> anyhow::Result<Vec<(NodeId, String)>> {
  let mut nodes = BTreeMap::new();
  for raw in opt.nodes.iter().chain(opt.bootstrap_nodes.iter()) {
    let (id, addr) = parse_node_kv(raw)?;
    nodes.insert(id, addr);
  }
  Ok(nodes.into_iter().collect())
}

pub(crate) fn known_control_nodes(
  members: &BTreeMap<NodeId, BasicNode>,
  self_id: &NodeId,
) -> Vec<NodeId> {
  members
    .keys()
    .filter(|node_id| *node_id != self_id)
    .cloned()
    .collect::<Vec<_>>()
}

/// True when this node is a VOTER of every raft group's persisted membership.
/// Learners are in `membership.nodes()` too, so a plain `get_node` check would
/// wrongly treat a learner as a control member.
pub(crate) fn local_openraft_voters_contain_self(
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

pub(crate) fn decide_startup_mode(
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
pub(crate) async fn remote_openraft_voters_contain_self(
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
pub(crate) async fn wait_for_any_peer_connected(
  network: &Libp2pNetworkFactory,
  timeout: Duration,
) -> bool {
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
pub(crate) async fn cleanup_removed_local_groups(
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

pub(crate) async fn fetch_remote_group_metrics(
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
pub(crate) async fn fetch_authoritative_group_metrics(
  group_id: &str,
  self_id: &NodeId,
  network: &Libp2pNetworkFactory,
) -> Option<crate::typ::RaftMetrics> {
  fetch_group_metrics_inner(group_id, self_id, network, true).await
}

pub(crate) async fn fetch_group_metrics_inner(
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

pub(crate) async fn maybe_bootstrap(
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

pub(crate) async fn maybe_initialize_bootstrap_openraft(
  registry: &crate::GroupRegistry,
  self_id: NodeId,
  self_addr: String,
  bootstrap_self: bool,
) -> anyhow::Result<()> {
  if !bootstrap_self {
    return Ok(());
  }

  let members = BTreeMap::from([(self_id, BasicNode { addr: self_addr })]);
  let groups = registry
    .all()
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

pub(crate) async fn advertise_openraft_kad(client: &Libp2pClient) {
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

pub(crate) async fn leave_openraft_kad(client: &Libp2pClient) -> anyhow::Result<()> {
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

use std::{
  collections::{HashMap, HashSet},
  net::IpAddr,
  sync::Arc,
  time::Duration,
};

use anyhow::Context;
use arc_swap::ArcSwap;
use async_trait::async_trait;
use libp2p::{Multiaddr, PeerId, multiaddr::Protocol};
use openraft::BasicNode;

use crate::{
  GroupId, NodeId, Unreachable,
  network::{
    peer_guard::{PeerRpcGuard, RpcKind},
    raft_bridge::{P2PNetworkFactory, P2PRaftNetwork},
    rpc::{RaftRpcRequest, RaftRpcResponse},
    swarm::{
      ClusterError, GOSSIPSUB_MESH_N_LOW, KvClient, Libp2pClient, SqliteSyncClient, TaskRpcClient,
      WasmSyncClient,
    },
  },
  proto::raft_kv::{RaftKvRequest, RaftKvResponse},
  sqlite_sync_rpc::{SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage},
  tasks::rpc::{TaskRpcRequestMessage, TaskRpcResponseMessage},
  wasm_sync::{WasmSyncRequest, WasmSyncResponse},
};

/// How long a raft RPC target stays in the proactive-reconnect set after the
/// last raft traffic to it. Long enough to ride out leader hiccups, short
/// enough that peers removed from a group membership stop being redialed.
const RAFT_PEER_PIN_TTL: Duration = Duration::from_secs(600);

/// Missed announcements tolerated before announce-based liveness lapses, so
/// a couple of lost gossip messages do not flap liveness.
const PEER_ALIVE_TTL_INTERVALS: u32 = 3;

/// Default announce-based liveness TTL, assuming the base announce interval.
/// Peers announcing at a stretched, cluster-size-scaled cadence carry their
/// interval in the announcement and get a proportional per-peer TTL instead.
const PEER_ALIVE_TTL: Duration =
  crate::app::NODE_ANNOUNCE_INTERVAL.saturating_mul(PEER_ALIVE_TTL_INTERVALS);

/// Why a peer must be kept connected by the proactive reconnect loop.
#[derive(Clone, Copy)]
enum PeerPin {
  /// Configured/bootstrap peer: keep connected for the process lifetime.
  Permanent,
  /// Raft RPC target: keep connected until the pin expires unrefreshed.
  Until(tokio::time::Instant),
}

#[derive(Clone)]
pub struct Libp2pNetworkFactory {
  client: Libp2pClient,
  kv_client: KvClient,
  sqlite_sync_client: SqliteSyncClient,
  task_rpc_client: TaskRpcClient,
  wasm_sync_client: WasmSyncClient,
  /// Snapshot-read address book: read on every RPC routing decision and
  /// liveness check, mutated only on (rare) registrations. `ArcSwap` makes
  /// the reads lock-free; writers copy-on-write via `rcu`.
  node_peers: Arc<ArcSwap<HashMap<NodeId, (PeerId, Multiaddr)>>>,
  /// Same trade-off: read per request / liveness probe, written only on
  /// connection establish/close events.
  connected_peers: Arc<ArcSwap<HashSet<PeerId>>>,
  /// Peers the reconnect loop must keep connected: configured members,
  /// bootstrap nodes, and recent raft RPC targets. Everything else connects
  /// on demand and is closed by the swarm idle-connection timeout, so the
  /// cluster does not degenerate into an O(N^2) full mesh.
  pinned_peers: Arc<tokio::sync::RwLock<HashMap<PeerId, PeerPin>>>,
  /// Last time a node-announce gossip from this peer was seen, plus the
  /// liveness TTL derived from the interval the peer announced. Used as a
  /// connection-free liveness signal for peers we deliberately stay
  /// disconnected from.
  peer_last_announce: Arc<tokio::sync::RwLock<HashMap<PeerId, (tokio::time::Instant, Duration)>>>,
  /// Per-target-peer bulkhead + circuit breaker over every outbound RPC
  /// path, so one slow or dead peer cannot absorb unbounded in-flight
  /// requests or shared dispatch capacity.
  peer_guard: Arc<PeerRpcGuard>,
  group_id: Option<GroupId>,
  local_peer_id: PeerId,
}

impl Libp2pNetworkFactory {
  pub fn new(
    client: Libp2pClient,
    kv_client: KvClient,
    sqlite_sync_client: SqliteSyncClient,
    task_rpc_client: TaskRpcClient,
    wasm_sync_client: WasmSyncClient,
    local_peer_id: PeerId,
  ) -> Self {
    Self {
      client,
      kv_client,
      sqlite_sync_client,
      task_rpc_client,
      wasm_sync_client,
      node_peers: Arc::new(ArcSwap::from_pointee(HashMap::new())),
      connected_peers: Arc::new(ArcSwap::from_pointee(HashSet::new())),
      pinned_peers: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
      peer_last_announce: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
      peer_guard: Arc::new(PeerRpcGuard::default()),
      group_id: None,
      local_peer_id,
    }
  }

  pub fn local_peer_id(&self) -> PeerId {
    self.local_peer_id
  }

  pub fn with_group(&self, group_id: GroupId) -> Self {
    Self {
      client: self.client.clone(),
      kv_client: self.kv_client.clone(),
      sqlite_sync_client: self.sqlite_sync_client.clone(),
      task_rpc_client: self.task_rpc_client.clone(),
      wasm_sync_client: self.wasm_sync_client.clone(),
      node_peers: self.node_peers.clone(),
      connected_peers: self.connected_peers.clone(),
      pinned_peers: self.pinned_peers.clone(),
      peer_last_announce: self.peer_last_announce.clone(),
      peer_guard: self.peer_guard.clone(),
      group_id: Some(group_id),
      local_peer_id: self.local_peer_id,
    }
  }

  pub async fn register_node(&self, node_id: NodeId, addr: &str) -> anyhow::Result<()> {
    let (peer, maddr) = parse_p2p_addr(addr)?;
    let should_dial = self
      .register_configured_node_addr(node_id.clone(), peer, maddr.clone())
      .await;
    if peer == self.local_peer_id {
      tracing::warn!(
        node_id = %node_id,
        peer = %peer,
        addr = %maddr,
        "skip self dial in register_node"
      );
      return Ok(());
    }
    if should_dial {
      self.client.dial(maddr).await;
    }
    Ok(())
  }

  async fn register_configured_node_addr(
    &self,
    node_id: NodeId,
    peer: PeerId,
    addr: Multiaddr,
  ) -> bool {
    // rcu may retry the closure under write contention, so all decisions are
    // captured in cells and the side effects (logging) run once afterwards,
    // based on the transition that actually got applied.
    let previous = std::cell::RefCell::new(None::<(PeerId, Multiaddr)>);
    let changed = std::cell::Cell::new(false);
    self.node_peers.rcu(|map| {
      let mut next = HashMap::clone(map);
      match next.get(&node_id) {
        Some((stored_peer, stored_addr)) if *stored_peer == peer && *stored_addr == addr => {
          previous.replace(None);
          changed.set(false);
        }
        stored => {
          previous.replace(stored.cloned());
          next.insert(node_id.clone(), (peer, addr.clone()));
          changed.set(true);
        }
      }
      next
    });

    if !changed.get() {
      return false;
    }
    match previous.into_inner() {
      Some((stored_peer, stored_addr)) => tracing::info!(
        node_id = %node_id,
        peer = %peer,
        addr = %addr,
        stored_peer = %stored_peer,
        stored_addr = %stored_addr,
        "registered configured libp2p node address"
      ),
      None => tracing::info!(
        node_id = %node_id,
        peer = %peer,
        addr = %addr,
        "registered configured libp2p node"
      ),
    }
    true
  }

  pub async fn register_discovered_peer(&self, peer: PeerId, addr: Multiaddr) -> bool {
    if peer == self.local_peer_id {
      return false;
    }

    let node_id = NodeId::from(peer);
    let addr = ensure_p2p_addr(addr, peer);
    if is_undialable_discovered_addr(&addr) {
      tracing::debug!(
        node_id = %node_id,
        peer = %peer,
        addr = %addr,
        "ignore undialable discovered peer address"
      );
      return false;
    }

    #[derive(Clone, Copy)]
    enum ScanOutcome {
      /// No entry for this peer; fall through to plain registration.
      NotFound,
      Keep,
      Ignore,
      Updated,
    }
    let outcome = std::cell::Cell::new(ScanOutcome::NotFound);
    self.node_peers.rcu(|map| {
      let mut next = HashMap::clone(map);
      outcome.set(ScanOutcome::NotFound);
      for (_, (stored_peer, stored_addr)) in next.iter_mut() {
        if *stored_peer != peer {
          continue;
        }

        if *stored_addr == addr {
          outcome.set(ScanOutcome::Keep);
        } else if !should_use_discovered_addr(stored_addr, &addr) {
          outcome.set(ScanOutcome::Ignore);
        } else if is_unspecified_addr(stored_addr) {
          *stored_addr = addr.clone();
          outcome.set(ScanOutcome::Updated);
        } else {
          outcome.set(ScanOutcome::Keep);
        }
        break;
      }
      next
    });

    match outcome.get() {
      ScanOutcome::NotFound => self.register_node_addr(node_id, peer, addr).await,
      ScanOutcome::Keep => true,
      ScanOutcome::Ignore => {
        tracing::debug!(
          node_id = %node_id,
          peer = %peer,
          addr = %addr,
          "ignoring discovered peer address"
        );
        false
      }
      ScanOutcome::Updated => {
        tracing::info!(
          node_id = %node_id,
          peer = %peer,
          addr = %addr,
          "updating unspecified peer address from discovery"
        );
        true
      }
    }
  }

  async fn register_node_addr(&self, node_id: NodeId, peer: PeerId, addr: Multiaddr) -> bool {
    #[derive(Clone, Copy)]
    enum Outcome {
      Unchanged,
      IgnoredLoopback,
      Registered,
    }
    let new_is_loopback = is_loopback_addr(&addr);
    let outcome = std::cell::Cell::new(Outcome::Unchanged);
    self.node_peers.rcu(|map| {
      let mut next = HashMap::clone(map);
      match next.get(&node_id) {
        Some((stored_peer, stored_addr)) if *stored_peer == peer && *stored_addr == addr => {
          outcome.set(Outcome::Unchanged);
        }
        Some((stored_peer, stored_addr))
          if *stored_peer == peer && !is_loopback_addr(stored_addr) && new_is_loopback =>
        {
          outcome.set(Outcome::IgnoredLoopback);
        }
        _ => {
          next.insert(node_id.clone(), (peer, addr.clone()));
          outcome.set(Outcome::Registered);
        }
      }
      next
    });
    match outcome.get() {
      Outcome::Unchanged => false,
      Outcome::IgnoredLoopback => {
        tracing::debug!(
          node_id = %node_id,
          peer = %peer,
          addr = %addr,
          "ignore loopback addr because a non-loopback addr is already known"
        );
        false
      }
      Outcome::Registered => {
        tracing::info!(
          node_id = %node_id,
          peer = %peer,
          addr = %addr,
          "registered libp2p node"
        );
        true
      }
    }
  }

  pub async fn update_peer_addr_from_mdns(&self, peer: PeerId, addr: Multiaddr) -> bool {
    #[derive(Clone, Copy)]
    enum Outcome {
      NotFound,
      Keep,
      Ignore,
      Updated,
    }
    let candidate = ensure_p2p_addr(addr, peer);
    let outcome = std::cell::Cell::new(Outcome::NotFound);
    self.node_peers.rcu(|map| {
      let mut next = HashMap::clone(map);
      outcome.set(Outcome::NotFound);
      for (_, (stored_peer, stored_addr)) in next.iter_mut() {
        if *stored_peer != peer {
          continue;
        }

        if candidate == *stored_addr {
          outcome.set(Outcome::Keep);
        } else if !should_use_discovered_addr(stored_addr, &candidate) {
          outcome.set(Outcome::Ignore);
        } else if is_unspecified_addr(stored_addr) {
          *stored_addr = candidate.clone();
          outcome.set(Outcome::Updated);
        } else {
          outcome.set(Outcome::Keep);
        }
        break;
      }
      next
    });

    match outcome.get() {
      Outcome::NotFound => !is_undialable_discovered_addr(&candidate),
      Outcome::Keep => true,
      Outcome::Ignore => {
        tracing::debug!(
          peer = %peer,
          addr = %candidate,
          "ignoring discovered peer address"
        );
        false
      }
      Outcome::Updated => {
        tracing::info!(
          peer = %peer,
          addr = %candidate,
          "updating unspecified peer address from mdns"
        );
        true
      }
    }
  }

  /// Register a peer address learned from a node-announce gossip message.
  /// `announce_interval_ms` is the cadence the sender declared; 0 means an
  /// old sender that announces at the default interval.
  ///
  /// Unlike [`register_node`](Self::register_node) this never dials: an
  /// announcement means "this node exists and is alive", not "open a
  /// connection to it". Dialing here would make every node connect to every
  /// announcer — an O(N^2) full mesh. Connections to announced peers are
  /// opened on demand by the RPC paths instead.
  ///
  /// Announce volume is O(cluster size), so the steady-state case — the node
  /// is already registered under the same address — must stay cheap: it only
  /// takes the address-book read lock, never the write lock the RPC paths
  /// contend on.
  pub async fn register_announced_node(
    &self,
    node_id: NodeId,
    addr: &str,
    announce_interval_ms: u64,
  ) -> anyhow::Result<()> {
    let (peer, maddr) = parse_p2p_addr(addr)?;
    if peer == self.local_peer_id {
      return Ok(());
    }
    self
      .mark_peer_announced(peer, peer_alive_ttl(announce_interval_ms))
      .await;
    let unchanged = self
      .node_peers
      .load()
      .get(&node_id)
      .is_some_and(|(stored_peer, stored_addr)| *stored_peer == peer && *stored_addr == maddr);
    if !unchanged {
      self
        .register_configured_node_addr(node_id, peer, maddr)
        .await;
    }
    Ok(())
  }

  /// Keep this peer in the proactive-reconnect set for the process lifetime.
  /// Used for explicitly configured members and bootstrap nodes.
  pub async fn pin_peer(&self, peer: PeerId) {
    if peer == self.local_peer_id {
      return;
    }
    self
      .pinned_peers
      .write()
      .await
      .insert(peer, PeerPin::Permanent);
  }

  /// Keep this peer in the proactive-reconnect set for [`RAFT_PEER_PIN_TTL`].
  /// Refreshed on every raft RPC, so active group members stay connected and
  /// removed members age out instead of being redialed forever.
  pub async fn pin_raft_peer(&self, peer: PeerId) {
    if peer == self.local_peer_id {
      return;
    }
    let until = tokio::time::Instant::now() + RAFT_PEER_PIN_TTL;
    let mut pins = self.pinned_peers.write().await;
    match pins.get(&peer) {
      Some(PeerPin::Permanent) => {}
      _ => {
        pins.insert(peer, PeerPin::Until(until));
      }
    }
  }

  async fn mark_peer_announced(&self, peer: PeerId, ttl: Duration) {
    self
      .peer_last_announce
      .write()
      .await
      .insert(peer, (tokio::time::Instant::now(), ttl));
  }

  /// Whether this peer should be treated as running. True when a direct
  /// connection exists OR its last node-announce gossip is fresh. Liveness
  /// consumers (membership guard, task scheduler, known-nodes pruner) must
  /// use this instead of raw connectedness: with on-demand connections a
  /// healthy but idle peer is intentionally not connected.
  pub async fn is_peer_alive(&self, peer: &PeerId) -> bool {
    if self.is_peer_connected(peer).await {
      return true;
    }
    self
      .peer_last_announce
      .read()
      .await
      .get(peer)
      .is_some_and(|(at, ttl)| at.elapsed() < *ttl)
  }

  /// Health of the local gossipsub mesh for `topic`, in `0.0..=1.0`:
  /// the ratio of actual mesh peers to the expected mesh degree
  /// (`min(mesh_n_low, subscribed peers)`). `1.0` means the mesh is at or
  /// above its maintenance low-water mark (or there is nothing to mesh
  /// with); values below `1.0` mean gossip delivery — and therefore the
  /// announce-based liveness signal — may be unreliable. `None` when the
  /// swarm report is unavailable or the topic is not subscribed.
  pub async fn gossipsub_mesh_health(&self, topic: &str) -> Option<f64> {
    let report = self.client.libp2p_info().await.ok()?;
    let topic_hash = libp2p::gossipsub::IdentTopic::new(topic).hash().to_string();
    let topic_report = report
      .gossipsub_topics
      .iter()
      .find(|report| report.topic == topic_hash)?;
    let expected = topic_report.subscribed_peers.min(GOSSIPSUB_MESH_N_LOW);
    if expected == 0 {
      return Some(1.0);
    }
    Some((topic_report.mesh_peers as f64 / expected as f64).clamp(0.0, 1.0))
  }

  pub async fn known_nodes(&self) -> Vec<(NodeId, PeerId, Multiaddr)> {
    self
      .node_peers
      .load()
      .iter()
      .map(|(id, (peer, addr))| (id.clone(), *peer, addr.clone()))
      .collect()
  }

  /// Size of the known-nodes address book. Used by the announcer to scale
  /// its interval with cluster size without cloning the whole map.
  pub async fn known_nodes_count(&self) -> usize {
    self.node_peers.load().len()
  }

  /// Known nodes the reconnect loop should proactively keep connected:
  /// pinned peers only. Expired raft pins are pruned on the way.
  pub async fn reconnect_targets(&self) -> Vec<(NodeId, PeerId, Multiaddr)> {
    let now = tokio::time::Instant::now();
    {
      let mut pins = self.pinned_peers.write().await;
      pins.retain(|_, pin| match pin {
        PeerPin::Permanent => true,
        PeerPin::Until(until) => *until > now,
      });
    }
    let pins = self.pinned_peers.read().await;
    let map = self.node_peers.load();
    map
      .iter()
      .filter(|(_, (peer, _))| pins.contains_key(peer))
      .map(|(id, (peer, addr))| (id.clone(), *peer, addr.clone()))
      .collect()
  }

  /// Drop a node from the known-nodes address book. Used to prune nodes that
  /// stayed disconnected past the healing timeout; a returning node is
  /// re-registered through mdns discovery or its bootstrap dial.
  pub async fn remove_known_node(&self, node_id: &NodeId) -> bool {
    let removed = std::cell::RefCell::new(None);
    self.node_peers.rcu(|map| {
      let mut next = HashMap::clone(map);
      removed.replace(next.remove(node_id));
      next
    });
    let Some((peer, _)) = removed.into_inner() else {
      return false;
    };
    self.pinned_peers.write().await.remove(&peer);
    self.peer_last_announce.write().await.remove(&peer);
    true
  }

  pub async fn set_peer_connected(&self, peer: PeerId) {
    if peer == self.local_peer_id {
      return;
    }
    self.connected_peers.rcu(|set| {
      let mut next = HashSet::clone(set);
      next.insert(peer);
      next
    });
  }

  pub async fn set_peer_disconnected(&self, peer: PeerId) {
    self.connected_peers.rcu(|set| {
      let mut next = HashSet::clone(set);
      next.remove(&peer);
      next
    });
  }

  pub async fn is_peer_connected(&self, peer: &PeerId) -> bool {
    if *peer == self.local_peer_id {
      return true;
    }
    self.connected_peers.load().contains(peer)
  }

  pub async fn publish_gossipsub(&self, topic: &str, data: Vec<u8>) -> Result<(), ClusterError> {
    self.client.publish_gossipsub(topic, data).await
  }

  pub async fn publish_openraft_snapshot(
    &self,
    group_id: String,
    registry: &crate::GroupRegistry,
  ) -> Result<String, ClusterError> {
    self
      .client
      .publish_openraft_snapshot(group_id, registry)
      .await
  }

  pub async fn request(
    &self,
    node_id: NodeId,
    req: RaftRpcRequest,
  ) -> Result<RaftRpcResponse, Unreachable> {
    self.request_ref(&node_id, req).await
  }

  async fn request_ref(
    &self,
    node_id: &NodeId,
    req: RaftRpcRequest,
  ) -> Result<RaftRpcResponse, Unreachable> {
    let (peer, addr) = self.peer_addr_for(node_id).await?;
    if peer == self.local_peer_id {
      return Err(Unreachable::new(&ClusterError::Network(format!(
        "self dial blocked: node_id={node_id}, peer={peer}"
      ))));
    }
    // Raft traffic to this peer means it is a live replication/vote target:
    // keep it in the proactive-reconnect set so raft latency never waits on
    // a fresh dial + handshake.
    self.pin_raft_peer(peer).await;
    self
      .guarded(RpcKind::Raft, peer, async {
        if let Err(err) = self.client.connect(peer, addr.clone()).await {
          if is_loopback_addr(&addr) || is_link_local_addr(&addr) {
            return Err(err);
          }
          tracing::warn!(
            node_id = %node_id,
            peer = %peer,
            addr = %addr,
            error = %err,
            "connect with configured address failed, trying any known address"
          );
          self.client.connect_any(peer).await?;
        }
        self.client.request(peer, req).await
      })
      .await
  }

  /// Run one outbound RPC under the per-(protocol, peer) guard: admission
  /// (bulkhead + circuit breaker) happens before any network activity, and
  /// the outcome feeds the circuit so consecutive failures to a peer
  /// eventually reject fast instead of piling up in-flight requests.
  async fn guarded<T>(
    &self,
    kind: RpcKind,
    peer: PeerId,
    rpc: impl Future<Output = Result<T, Unreachable>>,
  ) -> Result<T, Unreachable> {
    let permit = self
      .peer_guard
      .try_acquire(kind, peer)
      .map_err(|rejection| {
        Unreachable::new(&ClusterError::Network(format!("peer={peer}: {rejection}")))
      })?;
    let result = rpc.await;
    match &result {
      Ok(_) => permit.record_success(),
      Err(_) => permit.record_failure(),
    }
    result
  }

  pub async fn request_sqlite_sync(
    &self,
    node_id: NodeId,
    req: SqliteSyncRpcRequestMessage,
  ) -> Result<SqliteSyncRpcResponseMessage, Unreachable> {
    let (peer, addr) = self.peer_addr_for(&node_id).await?;
    if peer == self.local_peer_id {
      return Err(Unreachable::new(&ClusterError::Network(format!(
        "self dial blocked: node_id={node_id}, peer={peer}"
      ))));
    }
    self
      .guarded(RpcKind::SqliteSync, peer, async {
        if let Err(err) = self.sqlite_sync_client.connect(peer, addr.clone()).await {
          if is_loopback_addr(&addr) || is_link_local_addr(&addr) {
            return Err(err);
          }
          tracing::warn!(
            node_id = %node_id,
            peer = %peer,
            addr = %addr,
            error = %err,
            "connect with configured address failed, trying any known address for sqlite sync"
          );
          self.sqlite_sync_client.connect_any(peer).await?;
        }
        self.sqlite_sync_client.request(peer, req).await
      })
      .await
  }

  pub async fn request_task_rpc(
    &self,
    node_id: NodeId,
    req: TaskRpcRequestMessage,
  ) -> Result<TaskRpcResponseMessage, Unreachable> {
    let (peer, addr) = self.peer_addr_for(&node_id).await?;
    if peer == self.local_peer_id {
      return Err(Unreachable::new(&ClusterError::Network(format!(
        "self dial blocked: node_id={node_id}, peer={peer}"
      ))));
    }
    self
      .guarded(RpcKind::TaskRpc, peer, async {
        if let Err(err) = self.task_rpc_client.connect(peer, addr.clone()).await {
          if is_loopback_addr(&addr) || is_link_local_addr(&addr) {
            return Err(err);
          }
          tracing::warn!(
            node_id = %node_id,
            peer = %peer,
            addr = %addr,
            error = %err,
            "connect with configured address failed, trying any known address for task rpc"
          );
          self.task_rpc_client.connect_any(peer).await?;
        }
        self.task_rpc_client.request(peer, req).await
      })
      .await
  }

  pub async fn request_wasm_sync(
    &self,
    node_id: NodeId,
    req: WasmSyncRequest,
  ) -> Result<WasmSyncResponse, Unreachable> {
    let (peer, addr) = self.peer_addr_for(&node_id).await?;
    if peer == self.local_peer_id {
      return Err(Unreachable::new(&ClusterError::Network(format!(
        "self dial blocked: node_id={node_id}, peer={peer}"
      ))));
    }
    self
      .guarded(RpcKind::WasmSync, peer, async {
        if let Err(err) = self.wasm_sync_client.connect(peer, addr.clone()).await {
          if is_loopback_addr(&addr) || is_link_local_addr(&addr) {
            return Err(err);
          }
          tracing::warn!(
            node_id = %node_id,
            peer = %peer,
            addr = %addr,
            error = %err,
            "connect with configured address failed, trying any known address for wasm sync"
          );
          self.wasm_sync_client.connect_any(peer).await?;
        }
        self.wasm_sync_client.request(peer, req).await
      })
      .await
  }

  pub async fn request_kv(
    &self,
    node_id: NodeId,
    req: RaftKvRequest,
  ) -> Result<RaftKvResponse, Unreachable> {
    let (peer, addr) = self.peer_addr_for(&node_id).await?;
    if peer == self.local_peer_id {
      return Err(Unreachable::new(&ClusterError::Network(format!(
        "self dial blocked: node_id={node_id}, peer={peer}"
      ))));
    }
    self
      .guarded(RpcKind::Kv, peer, async {
        if let Err(err) = self.kv_client.connect(peer, addr.clone()).await {
          if is_loopback_addr(&addr) || is_link_local_addr(&addr) {
            return Err(err);
          }
          tracing::warn!(
            node_id = %node_id,
            peer = %peer,
            addr = %addr,
            error = %err,
            "connect with configured address failed, trying any known address for kv"
          );
          self.kv_client.connect_any(peer).await?;
        }
        self.kv_client.request(peer, req).await
      })
      .await
  }

  async fn peer_addr_for(&self, node_id: &NodeId) -> Result<(PeerId, Multiaddr), Unreachable> {
    self
      .node_peers
      .load()
      .get(node_id)
      .map(|(peer, addr)| (*peer, addr.clone()))
      .ok_or_else(|| {
        Unreachable::new(&ClusterError::Network(format!(
          "unknown target node_id={node_id}"
        )))
      })
  }
}

pub struct Libp2pRaftNetwork {
  target: NodeId,
  factory: Libp2pNetworkFactory,
  group_id: GroupId,
}

#[async_trait]
impl P2PNetworkFactory for Libp2pNetworkFactory {
  type Network = Libp2pRaftNetwork;

  async fn new_p2p_client(&self, target: NodeId, target_info: &BasicNode) -> Self::Network {
    // Best-effort registration (fails only on a malformed addr): the RPC
    // path re-resolves the peer per request, so a failure here just means
    // the first request dials from older address-book state.
    let _ = self.register_node(target.clone(), &target_info.addr).await;
    if let Ok((peer, _)) = parse_p2p_addr(&target_info.addr) {
      self.pin_raft_peer(peer).await;
    }
    let group_id = self
      .group_id
      .clone()
      .expect("group_id required for raft network");

    Libp2pRaftNetwork {
      target,
      factory: self.clone(),
      group_id,
    }
  }
}

#[async_trait]
impl P2PRaftNetwork for Libp2pRaftNetwork {
  fn target(&self) -> &NodeId {
    &self.target
  }

  fn group_id(&self) -> &GroupId {
    &self.group_id
  }

  async fn send_request(
    &self,
    target: &NodeId,
    request: RaftRpcRequest,
  ) -> Result<RaftRpcResponse, Unreachable> {
    self.factory.request_ref(target, request).await
  }
}

/// Liveness TTL for a peer that declared `announce_interval_ms` between its
/// announcements. Never below the default TTL, so old senders (0) and
/// senders on the base cadence keep today's behaviour.
fn peer_alive_ttl(announce_interval_ms: u64) -> Duration {
  Duration::from_millis(announce_interval_ms)
    .saturating_mul(PEER_ALIVE_TTL_INTERVALS)
    .max(PEER_ALIVE_TTL)
}

pub fn parse_p2p_addr(s: &str) -> anyhow::Result<(PeerId, Multiaddr)> {
  let addr: Multiaddr = s.parse().context("invalid multiaddr")?;

  let mut peer: Option<PeerId> = None;
  for p in addr.iter() {
    if let Protocol::P2p(pid) = p {
      peer = Some(pid);
      break;
    }
  }

  let peer = peer.ok_or_else(|| anyhow::anyhow!("multiaddr must include /p2p/<peerid>: {s}"))?;
  Ok((peer, addr))
}

fn ensure_p2p_addr(mut addr: Multiaddr, peer: PeerId) -> Multiaddr {
  if matches!(addr.iter().last(), Some(Protocol::P2p(_))) {
    return addr;
  }
  addr.push(Protocol::P2p(peer.into()));
  addr
}

fn is_loopback_addr(addr: &Multiaddr) -> bool {
  addr_ip(addr).is_some_and(|ip| ip.is_loopback()) || has_localhost_dns(addr)
}

fn is_unspecified_addr(addr: &Multiaddr) -> bool {
  addr_ip(addr).is_some_and(|ip| ip.is_unspecified())
}

fn is_link_local_addr(addr: &Multiaddr) -> bool {
  match addr_ip(addr) {
    Some(IpAddr::V4(ip)) => ip.octets()[0] == 169 && ip.octets()[1] == 254,
    Some(IpAddr::V6(ip)) => (ip.segments()[0] & 0xffc0) == 0xfe80,
    None => false,
  }
}

pub fn is_undialable_discovered_addr(addr: &Multiaddr) -> bool {
  is_unspecified_addr(addr) || is_link_local_addr(addr)
}

fn should_use_discovered_addr(stored_addr: &Multiaddr, candidate: &Multiaddr) -> bool {
  if is_unspecified_addr(candidate) {
    return false;
  }

  if is_loopback_addr(stored_addr) {
    return is_loopback_addr(candidate);
  }

  if is_loopback_addr(candidate) {
    return false;
  }

  if is_link_local_addr(candidate) {
    return false;
  }

  if !is_link_local_addr(stored_addr) && is_link_local_addr(candidate) {
    return false;
  }

  true
}

fn addr_ip(addr: &Multiaddr) -> Option<IpAddr> {
  for protocol in addr.iter() {
    match protocol {
      Protocol::Ip4(ip) => return Some(IpAddr::V4(ip)),
      Protocol::Ip6(ip) => return Some(IpAddr::V6(ip)),
      _ => {}
    }
  }
  None
}

fn has_localhost_dns(addr: &Multiaddr) -> bool {
  for protocol in addr.iter() {
    match protocol {
      Protocol::Dns(host) | Protocol::Dns4(host) | Protocol::Dns6(host) => {
        return host.eq_ignore_ascii_case("localhost");
      }
      _ => {}
    }
  }
  false
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use libp2p::identity;
  use tokio::sync::mpsc;

  use super::*;

  fn peer_id() -> PeerId {
    PeerId::from(identity::Keypair::generate_ed25519().public())
  }

  fn test_network(local_peer: PeerId) -> Libp2pNetworkFactory {
    let (tx, _rx) = mpsc::channel(4);
    let client = Libp2pClient::new(tx.clone(), Duration::from_secs(1));
    let kv_client = KvClient::new(tx.clone(), Duration::from_secs(1));
    let sqlite_sync_client = SqliteSyncClient::new(tx.clone(), Duration::from_secs(1));
    let task_rpc_client = TaskRpcClient::new(tx.clone(), Duration::from_secs(1));
    let wasm_sync_client = WasmSyncClient::new(tx, Duration::from_secs(1));
    Libp2pNetworkFactory::new(
      client,
      kv_client,
      sqlite_sync_client,
      task_rpc_client,
      wasm_sync_client,
      local_peer,
    )
  }

  #[tokio::test]
  async fn mdns_does_not_replace_configured_loopback_addr() {
    let local_peer = peer_id();
    let peer = peer_id();
    let network = test_network(local_peer);
    let node_id = NodeId::from("node-2");
    let configured_addr = format!("/ip4/127.0.0.1/tcp/4002/wss/p2p/{peer}");

    network
      .register_node(node_id.clone(), &configured_addr)
      .await
      .expect("register node");
    let use_discovered = network
      .update_peer_addr_from_mdns(peer, "/ip4/192.168.31.29/tcp/4002/wss".parse().unwrap())
      .await;

    let (_, stored_addr) = network.peer_addr_for(&node_id).await.expect("peer addr");
    assert!(!use_discovered);
    assert_eq!(stored_addr.to_string(), configured_addr);
  }

  #[tokio::test]
  async fn mdns_replaces_unspecified_configured_addr() {
    let local_peer = peer_id();
    let peer = peer_id();
    let network = test_network(local_peer);
    let node_id = NodeId::from("node-2");
    let configured_addr = format!("/ip4/0.0.0.0/tcp/4002/wss/p2p/{peer}");

    network
      .register_node(node_id.clone(), &configured_addr)
      .await
      .expect("register node");
    let use_discovered = network
      .update_peer_addr_from_mdns(peer, "/ip4/192.168.31.29/tcp/4002/wss".parse().unwrap())
      .await;

    let (_, stored_addr) = network.peer_addr_for(&node_id).await.expect("peer addr");
    assert!(use_discovered);
    assert_eq!(
      stored_addr.to_string(),
      format!("/ip4/192.168.31.29/tcp/4002/wss/p2p/{peer}")
    );
  }

  #[tokio::test]
  async fn discovered_loopback_does_not_reregister_known_normal_addr() {
    let local_peer = peer_id();
    let peer = peer_id();
    let network = test_network(local_peer);
    let node_id = NodeId::from(peer);
    let configured_addr = format!("/ip4/192.168.31.29/tcp/4002/wss/p2p/{peer}");

    network
      .register_node(node_id.clone(), &configured_addr)
      .await
      .expect("register node");
    let registered = network
      .register_discovered_peer(peer, "/ip4/127.0.0.1/tcp/4002/wss".parse().unwrap())
      .await;

    let (_, stored_addr) = network.peer_addr_for(&node_id).await.expect("peer addr");
    assert!(!registered);
    assert_eq!(stored_addr.to_string(), configured_addr);
  }

  #[tokio::test]
  async fn configured_loopback_replaces_previously_discovered_lan_addr() {
    let local_peer = peer_id();
    let peer = peer_id();
    let network = test_network(local_peer);
    let node_id = NodeId::from(peer);
    let configured_addr = format!("/ip4/127.0.0.1/tcp/4004/wss/p2p/{peer}");

    let registered = network
      .register_discovered_peer(peer, "/ip4/192.168.31.29/tcp/4004/wss".parse().unwrap())
      .await;
    network
      .register_node(node_id.clone(), &configured_addr)
      .await
      .expect("register node");

    let (_, stored_addr) = network.peer_addr_for(&node_id).await.expect("peer addr");
    assert!(registered);
    assert_eq!(stored_addr.to_string(), configured_addr);
  }

  #[tokio::test(start_paused = true)]
  async fn announced_peer_liveness_uses_declared_interval_ttl() {
    let local_peer = peer_id();
    let peer = peer_id();
    let network = test_network(local_peer);
    let node_id = NodeId::from(peer);
    let addr = format!("/ip4/192.168.31.29/tcp/4002/wss/p2p/{peer}");

    // Announced at a stretched cadence of 100s: TTL is 3x that, well past
    // the default TTL derived from the base interval.
    let interval_ms = 100_000;
    network
      .register_announced_node(node_id.clone(), &addr, interval_ms)
      .await
      .expect("register announced node");

    tokio::time::advance(PEER_ALIVE_TTL + Duration::from_secs(1)).await;
    assert!(
      network.is_peer_alive(&peer).await,
      "peer must stay alive past the default TTL when it declared a longer interval"
    );

    tokio::time::advance(Duration::from_millis(
      interval_ms * PEER_ALIVE_TTL_INTERVALS as u64,
    ))
    .await;
    assert!(
      !network.is_peer_alive(&peer).await,
      "peer must lapse after 3x its declared interval"
    );
  }

  #[tokio::test]
  async fn announced_node_registers_addr_and_repeat_is_noop() {
    let local_peer = peer_id();
    let peer = peer_id();
    let network = test_network(local_peer);
    let node_id = NodeId::from(peer);
    let addr = format!("/ip4/192.168.31.29/tcp/4002/wss/p2p/{peer}");

    network
      .register_announced_node(node_id.clone(), &addr, 0)
      .await
      .expect("register announced node");
    // Steady-state announce with an unchanged address takes the read-lock
    // fast path and must leave the stored address intact.
    network
      .register_announced_node(node_id.clone(), &addr, 0)
      .await
      .expect("repeat announced node");

    let (_, stored_addr) = network.peer_addr_for(&node_id).await.expect("peer addr");
    assert_eq!(stored_addr.to_string(), addr);
    assert!(network.is_peer_alive(&peer).await);
  }

  #[test]
  fn discovered_link_local_addr_is_not_used_for_normal_addr() {
    let peer = peer_id();
    let stored = format!("/ip4/192.168.31.29/tcp/4002/wss/p2p/{peer}")
      .parse()
      .unwrap();
    let candidate = format!("/ip4/169.254.173.129/tcp/4002/wss/p2p/{peer}")
      .parse()
      .unwrap();

    assert!(!should_use_discovered_addr(&stored, &candidate));
  }
}

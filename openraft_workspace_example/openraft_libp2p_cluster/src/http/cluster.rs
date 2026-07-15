use std::{
  collections::{BTreeMap, BTreeSet},
  sync::Arc,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::extract::{Query, State};
use libp2p::{Multiaddr, PeerId};
use openraft::{ServerState, async_runtime::WatchReceiver};
use prost::Message;
use serde::{Deserialize, Serialize};

use super::{
  AppState, ClusterQuery, Json, fetch_remote_metrics, openraft_group_ids, remote_server_state,
  resolve_group_id,
};
use crate::{
  GroupId, NodeId,
  network::{
    swarm::{
      GOSSIP_TOPIC, KAD_PROTOCOL, Libp2pSwarmReport, NODE_ANNOUNCE_TOPIC,
      OPENRAFT_CLUSTER_PROVIDER_KEY, UNIFIED_RPC_PROTOCOL,
    },
    transport::{Libp2pNetworkFactory, parse_p2p_addr},
  },
  proto::raft_kv::ChatMessage,
  store::ensure_linearizable_read,
};

#[derive(Serialize)]
pub(super) struct ClusterInfoResponse {
  node_id: NodeId,
  node_name: String,
  peer_id: String,
  listen: String,
  group_id: String,
  groups: Vec<String>,
  known_nodes: Vec<KnownNodeResponse>,
  raft_metrics: sonic_rs::Value,
  /// Full metrics fetched from the current leader when this node is not the
  /// leader of the queried group. OpenRaft only populates `heartbeat`,
  /// `replication`, and `last_quorum_acked` on the leader, so the local
  /// `raft_metrics` of a follower always reports them as null.
  #[serde(skip_serializing_if = "Option::is_none")]
  leader_raft_metrics: Option<sonic_rs::Value>,
  kv_data: Vec<KvPairResponse>,
  error: Option<String>,
}

#[derive(Serialize)]
struct KnownNodeResponse {
  node_id: NodeId,
  peer_id: String,
  addr: String,
  /// Whether a libp2p connection to this node is currently established.
  /// Crashed nodes stay in the known-node address book (so they can be
  /// re-dialed when they come back) but show up as `connected: false`.
  connected: bool,
}

#[derive(Serialize)]
pub(super) struct OpenRaftNodesResponse {
  ok: bool,
  local_node_id: NodeId,
  local_peer_id: String,
  /// One entry per raft group (all groups by default; a single one when
  /// filtered with `?group_id=`), each carrying its own leader and the
  /// per-node leader/follower/learner roles for that group.
  groups: Vec<OpenRaftGroupNodesResponse>,
  error: Option<String>,
}

#[derive(Serialize)]
struct OpenRaftGroupNodesResponse {
  group_id: String,
  ok: bool,
  leader_id: Option<NodeId>,
  /// The LOCAL node's raft server state in this group.
  raft_state: Option<String>,
  voters: usize,
  learners: usize,
  nodes: Vec<OpenRaftNodeResponse>,
  error: Option<String>,
}

#[derive(Serialize)]
struct OpenRaftNodeResponse {
  node_id: NodeId,
  peer_id: Option<String>,
  addr: String,
  role: String,
  connected: bool,
  is_local: bool,
  is_leader: bool,
  raft_state: Option<String>,
}

#[derive(Serialize)]
pub(super) struct Libp2pNodesResponse {
  ok: bool,
  local_node_id: NodeId,
  local_peer_id: String,
  listen: String,
  group_id: String,
  known_count: usize,
  connected_count: usize,
  openraft_member_count: usize,
  nodes: Vec<Libp2pNodeResponse>,
  error: Option<String>,
}

#[derive(Serialize)]
struct Libp2pNodeResponse {
  node_id: NodeId,
  peer_id: String,
  addr: String,
  connected: bool,
  is_local: bool,
  openraft_role: Option<String>,
}

#[derive(Serialize)]
struct KvPairResponse {
  key: String,
  value: String,
}

#[derive(Deserialize)]
pub(super) struct ChatRequest {
  text: String,
  from: Option<String>,
}

#[derive(Serialize)]
pub(super) struct ChatResponse {
  ok: bool,
  error: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct SyncSnapshotRequest {
  group_id: Option<String>,
}

#[derive(Serialize)]
pub(super) struct SyncSnapshotResponse {
  ok: bool,
  group_id: String,
  sync_group_id: Option<String>,
  error: Option<String>,
}

pub(super) async fn openraft_nodes(
  State(state): State<Arc<AppState>>,
  Query(query): Query<ClusterQuery>,
) -> Json<OpenRaftNodesResponse> {
  // No filter → every raft group, each with its own leader/follower/learner
  // attribution; `?group_id=` narrows to one group.
  let group_ids = match query.group_filter() {
    Some(group_id) => vec![group_id],
    None => openraft_group_ids(&state.registry),
  };

  if group_ids.is_empty() {
    return Json(OpenRaftNodesResponse {
      ok: false,
      local_node_id: state.node_id.clone(),
      local_peer_id: state.peer_id.clone(),
      groups: Vec::new(),
      error: Some("openraft groups are not initialized".to_string()),
    });
  }

  // The libp2p address book is group-independent; resolve it once.
  let known_nodes = known_nodes_by_id(&state.network).await;
  let mut groups = Vec::with_capacity(group_ids.len());
  for group_id in group_ids {
    groups.push(openraft_group_nodes(&state, group_id, &known_nodes).await);
  }

  let ok = groups.iter().all(|group| group.ok);
  Json(OpenRaftNodesResponse {
    ok,
    local_node_id: state.node_id.clone(),
    local_peer_id: state.peer_id.clone(),
    groups,
    error: (!ok).then(|| "one or more groups reported an error".to_string()),
  })
}

/// Membership snapshot of one raft group: its leader plus every member's
/// role (leader / follower / learner) as this group sees it.
async fn openraft_group_nodes(
  state: &AppState,
  group_id: String,
  known_nodes: &BTreeMap<NodeId, (PeerId, Multiaddr)>,
) -> OpenRaftGroupNodesResponse {
  let Some(group) = state.registry.get(&group_id) else {
    return OpenRaftGroupNodesResponse {
      error: Some(format!("unknown group_id={group_id}")),
      group_id,
      ok: false,
      leader_id: None,
      raft_state: None,
      voters: 0,
      learners: 0,
      nodes: Vec::new(),
    };
  };

  let metrics = group.raft.metrics().borrow_watched().clone();
  let membership = metrics.membership_config.membership();
  let voters = membership.voter_ids().collect::<BTreeSet<_>>();
  let learners = membership.learner_ids().collect::<BTreeSet<_>>();
  let mut nodes = Vec::new();

  for (node_id, node) in membership.nodes() {
    let is_leader = metrics.current_leader.as_ref() == Some(node_id);
    let role = if is_leader {
      "leader"
    } else if voters.contains(node_id) {
      "follower"
    } else if learners.contains(node_id) {
      "learner"
    } else {
      "member"
    }
    .to_string();

    let peer_id = known_nodes
      .get(node_id)
      .map(|(peer_id, _addr)| *peer_id)
      .or_else(|| peer_id_from_addr(&node.addr));
    let connected = match peer_id.as_ref() {
      Some(peer_id) => state.network.is_peer_connected(peer_id).await,
      None => false,
    };
    let is_local = node_id == &state.node_id;
    let raft_state = if is_local {
      Some(server_state_name(metrics.state))
    } else if connected {
      remote_server_state(&group_id, node_id, &state.network)
        .await
        .map(server_state_name)
    } else {
      None
    };

    nodes.push(OpenRaftNodeResponse {
      node_id: node_id.clone(),
      peer_id: peer_id.map(|peer_id| peer_id.to_string()),
      addr: node.addr.clone(),
      role,
      connected,
      is_local,
      is_leader,
      raft_state,
    });
  }

  nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));

  OpenRaftGroupNodesResponse {
    group_id,
    ok: true,
    leader_id: metrics.current_leader.clone(),
    raft_state: Some(server_state_name(metrics.state)),
    voters: voters.len(),
    learners: learners.len(),
    nodes,
    error: None,
  }
}

#[derive(Serialize)]
pub(super) struct Libp2pInfoResponse {
  ok: bool,
  /// Local node identity.
  node: Libp2pNodeIdentity,
  /// Ports derived from the configured addresses.
  ports: Libp2pPorts,
  /// Live swarm state (listeners, connections, kad mode, gossip meshes).
  swarm: Option<Libp2pSwarmReport>,
  /// The request-response RPC protocols this node speaks.
  request_response: Vec<RequestResponseProtocolInfo>,
  kad: KadInfo,
  gossipsub: GossipsubInfo,
  ping: PingInfo,
  mdns: MdnsInfo,
  /// The libp2p address book with liveness flags.
  known_nodes: Vec<KnownNodeResponse>,
  error: Option<String>,
}

#[derive(Serialize)]
struct Libp2pNodeIdentity {
  node_id: NodeId,
  node_name: String,
  peer_id: String,
  listen: String,
  http: String,
}

#[derive(Serialize)]
struct Libp2pPorts {
  p2p: Option<u16>,
  http: Option<u16>,
}

#[derive(Serialize)]
struct RequestResponseProtocolInfo {
  name: &'static str,
  protocol: &'static str,
  support: &'static str,
  codec: &'static str,
  used_for: &'static str,
}

#[derive(Serialize)]
struct KadInfo {
  protocol: &'static str,
  /// "Server" on control nodes (they advertise the cluster provider key),
  /// "Client" on workers/learners.
  mode: Option<String>,
  routing_table_peers: Option<usize>,
  provider_key: &'static str,
  provider_record_ttl_secs: u64,
  provider_publication_interval_secs: u64,
}

#[derive(Serialize)]
struct GossipsubInfo {
  topics: Vec<&'static str>,
  chat_topic: &'static str,
  node_announce_topic: &'static str,
  node_announce_interval_secs: u64,
  snapshot_sync_topic: &'static str,
  snapshot_sync_partial_messages: bool,
}

#[derive(Serialize)]
struct PingInfo {
  interval_secs: u64,
  timeout_secs: u64,
}

#[derive(Serialize)]
struct MdnsInfo {
  enabled: bool,
  used_for: &'static str,
}

fn multiaddr_port(addr: &str) -> Option<u16> {
  let addr: Multiaddr = addr.parse().ok()?;
  addr.iter().find_map(|proto| match proto {
    libp2p::multiaddr::Protocol::Tcp(port) | libp2p::multiaddr::Protocol::Udp(port) => Some(port),
    _ => None,
  })
}

pub(super) async fn libp2p_info(State(state): State<Arc<AppState>>) -> Json<Libp2pInfoResponse> {
  let (swarm, error) = match state.libp2p_client.libp2p_info().await {
    Ok(report) => (Some(report), None),
    Err(err) => (None, Some(format!("swarm info unavailable: {err}"))),
  };

  let mut known_nodes: Vec<KnownNodeResponse> = Vec::new();
  for (node_id, peer_id, addr) in state.network.known_nodes().await {
    let connected = state.network.is_peer_connected(&peer_id).await;
    known_nodes.push(KnownNodeResponse {
      node_id,
      peer_id: peer_id.to_string(),
      addr: addr.to_string(),
      connected,
    });
  }
  known_nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));

  Json(Libp2pInfoResponse {
    ok: error.is_none(),
    node: Libp2pNodeIdentity {
      node_id: state.node_id.clone(),
      node_name: state.node_name.clone(),
      peer_id: state.peer_id.clone(),
      listen: state.listen.clone(),
      http: state.http_addr.clone(),
    },
    ports: Libp2pPorts {
      p2p: multiaddr_port(&state.listen),
      http: state
        .http_addr
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse().ok()),
    },
    request_response: vec![RequestResponseProtocolInfo {
      name: "rpc",
      protocol: UNIFIED_RPC_PROTOCOL,
      support: "Full",
      codec: "kind-tagged envelope (UnifiedCodec): raft=json+lz4 snapshot frame, kv=protobuf, \
              sqlite-sync/task=json",
      used_for: "openraft AppendEntries/Vote/snapshot/JoinCluster/AddLearner/GetMetrics, KV ops, \
                 sqlite cache sync, task RPC",
    }],
    kad: KadInfo {
      protocol: KAD_PROTOCOL,
      mode: swarm.as_ref().map(|report| report.kad_mode.clone()),
      routing_table_peers: swarm.as_ref().map(|report| report.kad_routing_table_peers),
      provider_key: OPENRAFT_CLUSTER_PROVIDER_KEY,
      provider_record_ttl_secs: crate::app::OPENRAFT_KAD_PROVIDER_RECORD_TTL.as_secs(),
      provider_publication_interval_secs: crate::app::OPENRAFT_KAD_PROVIDER_PUBLICATION_INTERVAL
        .as_secs(),
    },
    gossipsub: GossipsubInfo {
      topics: vec![
        GOSSIP_TOPIC,
        NODE_ANNOUNCE_TOPIC,
        crate::network::openraft_sync::OPENRAFT_SYNC_TOPIC,
      ],
      chat_topic: GOSSIP_TOPIC,
      node_announce_topic: NODE_ANNOUNCE_TOPIC,
      node_announce_interval_secs: crate::app::NODE_ANNOUNCE_INTERVAL.as_secs(),
      snapshot_sync_topic: crate::network::openraft_sync::OPENRAFT_SYNC_TOPIC,
      snapshot_sync_partial_messages: true,
    },
    ping: PingInfo {
      interval_secs: crate::app::PING_INTERVAL.as_secs(),
      timeout_secs: crate::app::PING_TIMEOUT.as_secs(),
    },
    mdns: MdnsInfo {
      enabled: true,
      used_for: "LAN peer discovery feeding the known-nodes address book",
    },
    swarm,
    known_nodes,
    error,
  })
}

pub(super) async fn libp2p_nodes(
  State(state): State<Arc<AppState>>,
  Query(query): Query<ClusterQuery>,
) -> Json<Libp2pNodesResponse> {
  let group_id = query
    .group_filter()
    .unwrap_or_else(|| state.default_group.clone());
  let (roles, error) = match openraft_roles_by_node(&state.registry, &group_id) {
    Ok(roles) => (roles, None),
    Err(error) => (BTreeMap::new(), Some(error)),
  };
  let mut nodes = Vec::new();

  for (node_id, peer_id, addr) in state.network.known_nodes().await {
    let connected = state.network.is_peer_connected(&peer_id).await;
    nodes.push(Libp2pNodeResponse {
      openraft_role: roles.get(&node_id).cloned(),
      is_local: node_id == state.node_id,
      node_id,
      peer_id: peer_id.to_string(),
      addr: addr.to_string(),
      connected,
    });
  }

  nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
  let known_count = nodes.len();
  let connected_count = nodes.iter().filter(|node| node.connected).count();
  let openraft_member_count = nodes
    .iter()
    .filter(|node| node.openraft_role.is_some())
    .count();

  Json(Libp2pNodesResponse {
    ok: error.is_none(),
    local_node_id: state.node_id.clone(),
    local_peer_id: state.peer_id.clone(),
    listen: state.listen.clone(),
    group_id,
    known_count,
    connected_count,
    openraft_member_count,
    nodes,
    error,
  })
}

async fn known_nodes_by_id(
  network: &Libp2pNetworkFactory,
) -> BTreeMap<NodeId, (PeerId, Multiaddr)> {
  network
    .known_nodes()
    .await
    .into_iter()
    .map(|(node_id, peer_id, addr)| (node_id, (peer_id, addr)))
    .collect()
}

fn openraft_roles_by_node(
  registry: &crate::GroupRegistry,
  group_id: &str,
) -> Result<BTreeMap<NodeId, String>, String> {
  let Some(group) = registry.get(group_id) else {
    return if registry.all().is_none() {
      Err("openraft groups are not initialized".to_string())
    } else {
      Err(format!("unknown group_id={group_id}"))
    };
  };

  let metrics = group.raft.metrics().borrow_watched().clone();
  let membership = metrics.membership_config.membership();
  let mut roles = BTreeMap::new();
  for node_id in membership.voter_ids() {
    roles.insert(node_id, "voter".to_string());
  }
  for node_id in membership.learner_ids() {
    roles.insert(node_id, "learner".to_string());
  }
  if let Some(leader_id) = metrics.current_leader {
    roles.insert(leader_id, "leader".to_string());
  }
  Ok(roles)
}

fn peer_id_from_addr(addr: &str) -> Option<PeerId> {
  parse_p2p_addr(addr).ok().map(|(peer_id, _addr)| peer_id)
}

fn server_state_name(state: ServerState) -> String {
  format!("{state:?}")
}

pub(super) async fn cluster_info(
  State(state): State<Arc<AppState>>,
  Query(query): Query<ClusterQuery>,
) -> Json<ClusterInfoResponse> {
  let mut nodes: Vec<KnownNodeResponse> = Vec::new();
  for (node_id, peer_id, addr) in state.network.known_nodes().await {
    let connected = state.network.is_peer_connected(&peer_id).await;
    nodes.push(KnownNodeResponse {
      node_id,
      peer_id: peer_id.to_string(),
      addr: addr.to_string(),
      connected,
    });
  }

  nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));

  let group_id = query
    .group_filter()
    .unwrap_or_else(|| state.default_group.clone());

  let Some(global_groups) = state.registry.all() else {
    return Json(ClusterInfoResponse {
      node_id: state.node_id.clone(),
      node_name: state.node_name.clone(),
      peer_id: state.peer_id.clone(),
      listen: state.listen.clone(),
      group_id,
      groups: Vec::new(),
      known_nodes: nodes,
      raft_metrics: sonic_rs::Value::from_static_str("openraft groups are not initialized"),
      leader_raft_metrics: None,
      kv_data: Vec::new(),
      error: Some("openraft groups are not initialized".to_string()),
    });
  };

  let groups: Vec<String> = global_groups.keys().cloned().collect();

  let Some(group) = state.registry.get(&group_id) else {
    return Json(ClusterInfoResponse {
      node_id: state.node_id.clone(),
      node_name: state.node_name.clone(),
      peer_id: state.peer_id.clone(),
      listen: state.listen.clone(),
      group_id,
      groups,
      known_nodes: nodes,
      raft_metrics: sonic_rs::Value::from_static_str("unknown group"),
      leader_raft_metrics: None,
      kv_data: Vec::new(),
      error: Some("unknown group_id".to_string()),
    });
  };

  let metrics = group.raft.metrics().borrow_watched().clone();
  let raft_metrics = sonic_rs::to_value(&metrics)
    .unwrap_or_else(|err| sonic_rs::Value::copy_str(&format!("metrics serialize error: {err}")));

  // `heartbeat`, `replication`, and `last_quorum_acked` are leader-only
  // metrics in OpenRaft; on a follower they are always null. Fetch the
  // leader's metrics so any node can serve the full picture.
  let leader_raft_metrics = fetch_leader_metrics_value(&group_id, &metrics, &state.network).await;

  let mut kv_data = Vec::new();
  let allow_local_read = match tokio::time::timeout(
    Duration::from_millis(300),
    ensure_linearizable_read(&group.raft),
  )
  .await
  {
    Ok(Ok(())) => true,
    Ok(Err(err)) => {
      let is_forward = matches!(
        err.api_error(),
        Some(openraft::error::LinearizableReadError::ForwardToLeader(_))
      );
      if !is_forward {
        tracing::warn!("cluster_info read index failed: {err:?}");
      }
      is_forward
    }
    Err(_) => {
      tracing::warn!("cluster_info read index timeout");
      false
    }
  };
  if allow_local_read {
    match group.kv_data.entries().await {
      Ok(entries) => {
        for (key, value) in entries {
          kv_data.push(KvPairResponse { key, value });
        }
      }
      Err(err) => {
        tracing::warn!("cluster_info rocksdb kv read failed: {err:?}");
        kv_data.clear();
      }
    }
  }
  kv_data.sort_by(|a, b| a.key.cmp(&b.key));

  Json(ClusterInfoResponse {
    node_id: state.node_id.clone(),
    node_name: state.node_name.clone(),
    peer_id: state.peer_id.clone(),
    listen: state.listen.clone(),
    group_id,
    groups,
    known_nodes: nodes,
    raft_metrics,
    leader_raft_metrics,
    kv_data,
    error: None,
  })
}

/// When the local node is not the leader of `group_id`, fetch the current
/// leader's full metrics over libp2p so that leader-only fields are visible.
/// Returns `None` when the local node is the leader (local metrics already
/// carry the leader-only fields), the leader is unknown, or the fetch fails.
async fn fetch_leader_metrics_value(
  group_id: &str,
  metrics: &crate::typ::RaftMetrics,
  network: &Libp2pNetworkFactory,
) -> Option<sonic_rs::Value> {
  if metrics.state.is_leader() {
    return None;
  }
  let leader_id = metrics.current_leader.clone()?;

  let leader_metrics = match tokio::time::timeout(
    Duration::from_secs(2),
    fetch_remote_metrics(group_id, &leader_id, network),
  )
  .await
  {
    Ok(leader_metrics) => leader_metrics?,
    Err(_) => {
      tracing::debug!(
        group = group_id,
        leader = %leader_id,
        "timed out fetching leader metrics for cluster info"
      );
      return None;
    }
  };

  sonic_rs::to_value(&leader_metrics).ok()
}

pub(super) async fn send_chat(
  State(state): State<Arc<AppState>>,
  Json(req): Json<ChatRequest>,
) -> Json<ChatResponse> {
  let from = req.from.unwrap_or_else(|| state.node_name.clone());
  let ts = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis() as i64;
  let chat = ChatMessage {
    from,
    text: req.text,
    ts_unix_ms: ts,
  };

  let mut buf = Vec::new();
  if let Err(err) = chat.encode(&mut buf) {
    return Json(ChatResponse {
      ok: false,
      error: Some(format!("encode error: {err}")),
    });
  }

  match state.network.publish_gossipsub(GOSSIP_TOPIC, buf).await {
    Ok(()) => Json(ChatResponse {
      ok: true,
      error: None,
    }),
    Err(err) => Json(ChatResponse {
      ok: false,
      error: Some(err.to_string()),
    }),
  }
}

pub(super) async fn sync_snapshot(
  State(state): State<Arc<AppState>>,
  Json(req): Json<SyncSnapshotRequest>,
) -> Json<SyncSnapshotResponse> {
  let group_id = match resolve_group_id(state.as_ref(), req.group_id) {
    Ok(group_id) => group_id,
    Err(err) => {
      return Json(SyncSnapshotResponse {
        ok: false,
        group_id: state.default_group.clone(),
        sync_group_id: None,
        error: Some(err),
      });
    }
  };

  match state
    .network
    .publish_openraft_snapshot(group_id.clone())
    .await
  {
    Ok(sync_group_id) => Json(SyncSnapshotResponse {
      ok: true,
      group_id,
      sync_group_id: Some(sync_group_id),
      error: None,
    }),
    Err(err) => Json(SyncSnapshotResponse {
      ok: false,
      group_id,
      sync_group_id: None,
      error: Some(err.to_string()),
    }),
  }
}

/// Prometheus exposition endpoint (`GET /metrics`).
pub(super) async fn prometheus_metrics() -> String {
  crate::telemetry::prometheus_handle().render()
}

#[derive(serde::Serialize)]
pub(super) struct GroupsResponse {
  ok: bool,
  /// Live raft groups served by this node (from the group registry, not the
  /// static startup list).
  groups: Vec<String>,
  default_group: GroupId,
  initialized: bool,
}

/// `GET /groups`: discover the raft groups this node actually serves.
pub(super) async fn list_groups(State(state): State<Arc<AppState>>) -> Json<GroupsResponse> {
  let groups: Vec<String> = state
    .registry
    .all()
    .map(|groups| groups.keys().cloned().collect())
    .unwrap_or_default();
  Json(GroupsResponse {
    ok: true,
    initialized: !groups.is_empty(),
    default_group: state.default_group.clone(),
    groups,
  })
}

#[derive(serde::Serialize)]
pub(super) struct RuntimeConfigResponse {
  ok: bool,
  config: crate::runtime_config::RuntimeConfig,
  error: Option<String>,
}

/// `GET /config`: current hot-reloadable runtime configuration.
pub(super) async fn get_runtime_config() -> Json<RuntimeConfigResponse> {
  Json(RuntimeConfigResponse {
    ok: true,
    config: (*crate::runtime_config::current()).clone(),
    error: None,
  })
}

/// `POST /config`: partial update of the hot-reloadable configuration. Only
/// the fields present in the JSON body change; validation failures leave the
/// config untouched. This node only — cluster-wide changes are a matter of
/// POSTing to every node.
pub(super) async fn update_runtime_config(
  body: Json<crate::runtime_config::RuntimeConfigPatch>,
) -> Json<RuntimeConfigResponse> {
  match crate::runtime_config::apply_patch(body.0) {
    Ok(config) => Json(RuntimeConfigResponse {
      ok: true,
      config: (*config).clone(),
      error: None,
    }),
    Err(err) => Json(RuntimeConfigResponse {
      ok: false,
      config: (*crate::runtime_config::current()).clone(),
      error: Some(err),
    }),
  }
}

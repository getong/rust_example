use std::{
  collections::{BTreeMap, BTreeSet},
  net::SocketAddr,
  sync::Arc,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use axum::{
  Router,
  body::to_bytes,
  extract::{FromRequest, Query, Request, State},
  http::{
    StatusCode,
    header::{CONTENT_TYPE, HeaderMap, HeaderValue},
  },
  response::{IntoResponse, Response},
  routing::{get, post},
};
use libp2p::{Multiaddr, PeerId};
use openraft::{BasicNode, ServerState, async_runtime::WatchReceiver, log_id::RaftLogId};
use prost::Message;
use serde::{
  Deserialize, Deserializer, Serialize,
  de::{self, DeserializeOwned, Visitor},
};

use crate::{
  GroupId, NodeId,
  graphviz::{
    ClusterGraphGroup, ClusterGraphNode, ClusterGraphSnapshot, cluster_graph_dot, cluster_graph_svg,
  },
  groups,
  network::{
    openraft_dispatcher::process_kv_request,
    rpc::{AddLearnerRequest, RaftRpcOp, RaftRpcRequest, RaftRpcResponse},
    swarm::{
      GOSSIP_TOPIC, KAD_PROTOCOL, KvClient, Libp2pClient, Libp2pSwarmReport, NODE_ANNOUNCE_TOPIC,
      OPENRAFT_CLUSTER_PROVIDER_KEY, UNIFIED_RPC_PROTOCOL,
    },
    transport::{Libp2pNetworkFactory, parse_p2p_addr},
  },
  proto::raft_kv::{
    ChatMessage, DeleteValueRequest, RaftKvRequest, RaftKvResponse, SetValueRequest,
    UpdateValueRequest as ProtoUpdateValueRequest, raft_kv_request::Op as KvRequestOp,
    raft_kv_response::Op as KvResponseOp,
  },
  signal::ShutdownRx,
  sqlite_cache::{CachedValue, SqliteCache, pending_key, record_pending_key},
  store::ensure_linearizable_read,
  tasks::{
    TaskOpResult, TaskQueueMetrics, TaskRecord, WorkerLeaseRecord,
    handlers::{Email, TaskPayload},
    rpc::{ControlNodes, TaskRpc, TaskRpcRequest, TaskRpcResponse, TaskRpcService},
    worker::{call_read, submit_command},
  },
  types_kv::TaskRequest as StateCommand,
};

const HTTP_JSON_BODY_LIMIT: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

impl<T, S> FromRequest<S> for Json<T>
where
  T: DeserializeOwned,
  S: Send + Sync,
{
  type Rejection = Response;

  async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
    if !is_json_content_type(req.headers()) {
      return Err(
        (
          StatusCode::UNSUPPORTED_MEDIA_TYPE,
          "expected content-type application/json",
        )
          .into_response(),
      );
    }

    let bytes = to_bytes(req.into_body(), HTTP_JSON_BODY_LIMIT)
      .await
      .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()).into_response())?;
    sonic_rs::from_slice(&bytes)
      .map(Self)
      .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()).into_response())
  }
}

impl<T> IntoResponse for Json<T>
where
  T: Serialize,
{
  fn into_response(self) -> Response {
    match sonic_rs::to_vec(&self.0) {
      Ok(bytes) => (
        [(CONTENT_TYPE, HeaderValue::from_static("application/json"))],
        bytes,
      )
        .into_response(),
      Err(err) => (
        StatusCode::INTERNAL_SERVER_ERROR,
        [(
          CONTENT_TYPE,
          HeaderValue::from_static("text/plain; charset=utf-8"),
        )],
        err.to_string(),
      )
        .into_response(),
    }
  }
}

fn is_json_content_type(headers: &HeaderMap) -> bool {
  let Some(content_type) = headers.get(CONTENT_TYPE) else {
    return false;
  };
  let Ok(content_type) = content_type.to_str() else {
    return false;
  };
  content_type
    .split(';')
    .next()
    .map(str::trim)
    .is_some_and(|mime| {
      mime.eq_ignore_ascii_case("application/json")
        || mime
          .rsplit_once('+')
          .is_some_and(|(_, suffix)| suffix.eq_ignore_ascii_case("json"))
    })
}

#[derive(Clone)]
pub struct AppState {
  pub node_id: NodeId,
  pub node_name: String,
  pub peer_id: String,
  pub listen: String,
  pub http_addr: String,
  pub network: Libp2pNetworkFactory,
  pub kv_client: KvClient,
  pub libp2p_client: Libp2pClient,
  pub default_group: GroupId,
  pub task_frontend: TaskFrontend,
  pub sqlite_cache: Option<SqliteCache>,
  /// Injected raft group registry: handlers resolve groups through this
  /// instead of the process-wide global, so tests can serve an isolated set
  /// of groups.
  pub registry: crate::GroupRegistry,
}

/// How this node reaches the replicated task queue.
#[derive(Clone)]
pub enum TaskFrontend {
  /// Control node: submit directly through the local raft handle.
  Control,
  /// Worker node: go through the tarpc TaskRpc protocol to control nodes.
  Worker {
    control_nodes: Arc<tokio::sync::Mutex<ControlNodes>>,
  },
}

pub async fn serve(
  addr: SocketAddr,
  state: AppState,
  mut shutdown_rx: ShutdownRx,
) -> anyhow::Result<()> {
  let app = Router::new()
    .route("/cluster", get(cluster_info))
    .route("/openraft/nodes", get(openraft_nodes))
    .route("/libp2p/nodes", get(libp2p_nodes))
    .route("/libp2p/info", get(libp2p_info))
    .route("/cluster/openraft", get(openraft_nodes))
    .route("/cluster/libp2p", get(libp2p_nodes))
    .route("/openraft/membership/add", post(add_openraft_member))
    .route("/openraft/membership/remove", post(remove_openraft_member))
    .route("/graph", get(cluster_graph_page))
    .route("/graph.dot", get(cluster_graph_dot_response))
    .route("/graph.svg", get(cluster_graph_svg_response))
    .route("/chat", post(send_chat))
    .route("/sync/snapshot", post(sync_snapshot))
    .route("/tasks/email", post(push_email))
    .route("/tasks/push", post(push_task))
    .route("/tasks", get(list_tasks))
    .route("/tasks/workers", get(list_task_workers))
    .route("/tasks/metrics", get(task_metrics))
    .route("/write", post(set_value))
    .route("/update", post(update_value))
    .route("/delete", post(delete_value))
    .route("/cache/write", post(write_cached_value))
    .route("/cache/read", post(read_cached_value))
    .route("/sqlite/values", get(list_sqlite_values))
    .route("/metrics", get(prometheus_metrics))
    .route(
      "/config",
      get(get_runtime_config).post(update_runtime_config),
    )
    .route("/groups", get(list_groups))
    .with_state(Arc::new(state));

  let listener = tokio::net::TcpListener::bind(addr)
    .await
    .context("bind http")?;
  axum::serve(listener, app)
    .with_graceful_shutdown(async move {
      let _ = shutdown_rx.changed().await;
    })
    .await
    .context("serve http")?;
  Ok(())
}

#[derive(Serialize)]
struct ClusterInfoResponse {
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
struct OpenRaftNodesResponse {
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
struct Libp2pNodesResponse {
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

#[derive(Deserialize)]
struct RemoveOpenRaftMemberRequest {
  node_id: NodeId,
  group_id: Option<String>,
}

#[derive(Deserialize)]
struct AddOpenRaftMemberRequest {
  node_id: NodeId,
  addr: Option<String>,
  group_id: Option<String>,
  #[serde(default = "default_promote_openraft_member")]
  promote: bool,
  catch_up_timeout_secs: Option<u64>,
}

fn default_promote_openraft_member() -> bool {
  true
}

#[derive(Serialize)]
struct AddOpenRaftMemberResponse {
  ok: bool,
  target_node_id: NodeId,
  groups: Vec<AddOpenRaftMemberGroupResponse>,
  error: Option<String>,
}

#[derive(Serialize)]
struct AddOpenRaftMemberGroupResponse {
  group_id: String,
  ok: bool,
  before_voters: Vec<NodeId>,
  after_voters: Vec<NodeId>,
  leader_id: Option<NodeId>,
  learner_added: bool,
  promoted: bool,
  error: Option<String>,
}

#[derive(Serialize)]
struct RemoveOpenRaftMemberResponse {
  ok: bool,
  target_node_id: NodeId,
  groups: Vec<RemoveOpenRaftMemberGroupResponse>,
  error: Option<String>,
}

#[derive(Serialize)]
struct RemoveOpenRaftMemberGroupResponse {
  group_id: String,
  ok: bool,
  before_voters: Vec<NodeId>,
  after_voters: Vec<NodeId>,
  leader_id: Option<NodeId>,
  error: Option<String>,
}

#[derive(Serialize)]
struct KvPairResponse {
  key: String,
  value: String,
}

#[derive(Deserialize)]
struct WriteValueRequest {
  key: String,
  #[serde(deserialize_with = "string_or_number")]
  value: String,
  group_id: Option<String>,
  target_node_id: Option<NodeId>,
}

#[derive(Serialize)]
struct WriteValueResponse {
  target_node_id: Option<NodeId>,
  ok: bool,
  value: Option<String>,
  error: Option<String>,
}

#[derive(Deserialize)]
struct UpdateValueRequest {
  key: String,
  #[serde(deserialize_with = "string_or_number")]
  value: String,
  group_id: Option<String>,
  target_node_id: Option<NodeId>,
}

#[derive(Serialize)]
struct UpdateValueResponse {
  target_node_id: Option<NodeId>,
  ok: bool,
  value: Option<String>,
  error: Option<String>,
}

#[derive(Deserialize)]
struct DeleteValueRequestBody {
  key: String,
  group_id: Option<String>,
  target_node_id: Option<NodeId>,
}

#[derive(Serialize)]
struct DeleteValueResponseBody {
  target_node_id: Option<NodeId>,
  ok: bool,
  error: Option<String>,
}

#[derive(Deserialize)]
struct CacheWriteRequest {
  key: String,
  #[serde(deserialize_with = "string_or_number")]
  value: String,
  group_id: Option<String>,
  target_node_id: Option<NodeId>,
}

#[derive(Serialize)]
struct CacheWriteResponse {
  target_node_id: Option<NodeId>,
  ok: bool,
  pending_key: Option<String>,
  error: Option<String>,
}

#[derive(Deserialize)]
struct CacheReadRequest {
  key: String,
}

#[derive(Serialize)]
struct CacheReadResponse {
  ok: bool,
  found: bool,
  value: Option<String>,
  error: Option<String>,
}

#[derive(Serialize)]
struct SqliteValuesResponse {
  ok: bool,
  values: Vec<CachedValue>,
  error: Option<String>,
}

#[derive(Deserialize)]
struct ChatRequest {
  text: String,
  from: Option<String>,
}

#[derive(Serialize)]
struct ChatResponse {
  ok: bool,
  error: Option<String>,
}

#[derive(Deserialize)]
struct SyncSnapshotRequest {
  group_id: Option<String>,
}

#[derive(Serialize)]
struct SyncSnapshotResponse {
  ok: bool,
  group_id: String,
  sync_group_id: Option<String>,
  error: Option<String>,
}

#[derive(Deserialize)]
struct EmailRequest {
  to: String,
  /// Optional idempotency key: repeated pushes with the same key return the
  /// original task id with `deduplicated: true`.
  idem_key: Option<String>,
}

/// Generic task submission: `payload` is the kind-tagged JSON handed to the
/// worker-side handler registry (e.g. `{"kind":"digest","data":"x"}`).
#[derive(Deserialize)]
struct PushTaskRequest {
  payload: sonic_rs::Value,
  idem_key: Option<String>,
  /// Schedule the task `delay_secs` into the future (run_at = now + delay).
  #[serde(default)]
  delay_secs: u64,
}

#[derive(Serialize)]
struct EmailResponse {
  ok: bool,
  task_id: Option<String>,
  deduplicated: Option<bool>,
  error: Option<String>,
}

#[derive(Serialize)]
struct TasksResponse {
  ok: bool,
  tasks: Vec<TaskRecord>,
  error: Option<String>,
}

#[derive(Serialize)]
struct TaskWorkersResponse {
  ok: bool,
  workers: Vec<WorkerLeaseRecord>,
  error: Option<String>,
}

#[derive(Serialize)]
struct TaskMetricsResponse {
  ok: bool,
  metrics: Option<TaskQueueMetrics>,
  error: Option<String>,
}

#[derive(Deserialize)]
struct ClusterQuery {
  #[serde(alias = "group")]
  group_id: Option<String>,
}

impl ClusterQuery {
  /// The requested group filter; an EMPTY `group_id=` parameter (emitted by
  /// the graph page's "all groups" links) counts as "no filter".
  fn group_filter(self) -> Option<String> {
    self.group_id.filter(|group_id| !group_id.is_empty())
  }
}

async fn cluster_graph_page(
  State(state): State<Arc<AppState>>,
  Query(query): Query<ClusterQuery>,
) -> Response {
  let snapshot = cluster_graph_snapshot(state.as_ref(), query).await;
  let body = render_cluster_graph_page(&snapshot);
  (
    StatusCode::OK,
    [(
      CONTENT_TYPE,
      HeaderValue::from_static("text/html; charset=utf-8"),
    )],
    body,
  )
    .into_response()
}

async fn cluster_graph_dot_response(
  State(state): State<Arc<AppState>>,
  Query(query): Query<ClusterQuery>,
) -> Response {
  let snapshot = cluster_graph_snapshot(state.as_ref(), query).await;
  (
    StatusCode::OK,
    [(
      CONTENT_TYPE,
      HeaderValue::from_static("text/vnd.graphviz; charset=utf-8"),
    )],
    cluster_graph_dot(&snapshot),
  )
    .into_response()
}

async fn cluster_graph_svg_response(
  State(state): State<Arc<AppState>>,
  Query(query): Query<ClusterQuery>,
) -> Response {
  let snapshot = cluster_graph_snapshot(state.as_ref(), query).await;
  match tokio::task::spawn_blocking(move || cluster_graph_svg(&snapshot)).await {
    Ok(Ok(svg)) => (
      StatusCode::OK,
      [(CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"))],
      svg,
    )
      .into_response(),
    Ok(Err(err)) => (
      StatusCode::INTERNAL_SERVER_ERROR,
      [(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
      )],
      format!("render graphviz svg: {err}"),
    )
      .into_response(),
    Err(err) => (
      StatusCode::INTERNAL_SERVER_ERROR,
      [(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
      )],
      format!("join graphviz render task: {err}"),
    )
      .into_response(),
  }
}

async fn cluster_graph_snapshot(state: &AppState, query: ClusterQuery) -> ClusterGraphSnapshot {
  // No (or empty) filter → render every raft group; each group contributes
  // its own leader and replication edges to the same physical topology.
  let selected = query.group_filter();
  let all_group_ids = openraft_group_ids(&state.registry);
  let render_ids = match selected.clone() {
    Some(group_id) => vec![group_id],
    None => all_group_ids.clone(),
  };

  let error = render_ids
    .is_empty()
    .then(|| "openraft groups are not initialized".to_string());

  let groups: Vec<ClusterGraphGroup> = render_ids
    .into_iter()
    .map(|group_id| match state.registry.get(&group_id) {
      Some(group) => ClusterGraphGroup {
        group_id,
        metrics: Some(group.raft.metrics().borrow_watched().clone()),
        error: None,
      },
      None => ClusterGraphGroup {
        error: Some(format!("unknown group_id={group_id}")),
        group_id,
        metrics: None,
      },
    })
    .collect();

  let known_nodes = state.network.known_nodes().await;
  let mut nodes = Vec::with_capacity(known_nodes.len());
  for (node_id, peer_id, addr) in known_nodes {
    let connected = state.network.is_peer_connected(&peer_id).await;
    let mut server_states = BTreeMap::new();
    if node_id == state.node_id {
      for group in &groups {
        if let Some(metrics) = group.metrics.as_ref() {
          server_states.insert(group.group_id.clone(), metrics.state);
        }
      }
    }
    nodes.push(ClusterGraphNode {
      node_id,
      peer_id: peer_id.to_string(),
      addr: addr.to_string(),
      connected,
      server_states,
    });
  }

  // Live server states of remote nodes, fetched per (group, node) pair in
  // parallel so the page pays one RPC round-trip, not groups x nodes.
  let mut probes = Vec::new();
  for group in &groups {
    if group.metrics.is_none() {
      continue;
    }
    for (index, node) in nodes.iter().enumerate() {
      if node.node_id == state.node_id || !node.connected {
        continue;
      }
      let group_id = group.group_id.clone();
      let node_id = node.node_id.clone();
      let network = state.network.clone();
      probes.push(async move {
        let server_state = remote_server_state(&group_id, &node_id, &network).await;
        (index, group_id, server_state)
      });
    }
  }
  for (index, group_id, server_state) in futures::future::join_all(probes).await {
    if let Some(server_state) = server_state {
      nodes[index].server_states.insert(group_id, server_state);
    }
  }

  ClusterGraphSnapshot {
    self_node_id: state.node_id.clone(),
    self_peer_id: state.peer_id.clone(),
    self_listen: state.listen.clone(),
    selected,
    all_group_ids,
    groups,
    nodes,
    error,
  }
}

async fn remote_server_state(
  group_id: &str,
  node_id: &NodeId,
  network: &Libp2pNetworkFactory,
) -> Option<ServerState> {
  fetch_remote_metrics(group_id, node_id, network)
    .await
    .map(|metrics| metrics.state)
}

async fn fetch_remote_metrics(
  group_id: &str,
  node_id: &NodeId,
  network: &Libp2pNetworkFactory,
) -> Option<crate::typ::RaftMetrics> {
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
    Ok(RaftRpcResponse::GetMetrics(metrics)) => Some(metrics),
    Ok(RaftRpcResponse::Error(message)) => {
      tracing::debug!(
        group = group_id,
        node_id = %node_id,
        error = %message,
        "remote openraft metrics request returned error"
      );
      None
    }
    Ok(other) => {
      tracing::debug!(
        group = group_id,
        node_id = %node_id,
        response = ?other,
        "unexpected remote openraft metrics response"
      );
      None
    }
    Err(err) => {
      tracing::debug!(
        group = group_id,
        node_id = %node_id,
        error = ?err,
        "remote openraft metrics request failed"
      );
      None
    }
  }
}

fn render_cluster_graph_page(snapshot: &ClusterGraphSnapshot) -> String {
  let all_selected = if snapshot.selected.is_none() {
    " selected"
  } else {
    ""
  };
  let group_options = std::iter::once(format!(
    "<option value=\"\"{all_selected}>all groups</option>"
  ))
  .chain(snapshot.all_group_ids.iter().map(|group| {
    let selected = if snapshot.selected.as_deref() == Some(group.as_str()) {
      " selected"
    } else {
      ""
    };
    format!(
      "<option value=\"{}\"{}>{}</option>",
      html_escape(group),
      selected,
      html_escape(group)
    )
  }))
  .collect::<String>();
  let status = snapshot
    .error
    .iter()
    .chain(
      snapshot
        .groups
        .iter()
        .filter_map(|group| group.error.as_ref()),
    )
    .map(|err| format!("<p class=\"error\">{}</p>", html_escape(err)))
    .collect::<String>();
  // Empty group_id in links/SVG means "all groups" (the snapshot treats an
  // empty filter as no filter).
  let selected_param = snapshot.selected.as_deref().unwrap_or("");
  format!(
    r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta http-equiv="refresh" content="5">
  <title>libp2p openraft graph</title>
  <style>
    :root {{
      color-scheme: light;
      --bg: #f8fafc;
      --panel: #ffffff;
      --ink: #0f172a;
      --muted: #64748b;
      --line: #cbd5e1;
      --accent: #0f766e;
      --danger: #b91c1c;
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      min-height: 100vh;
      background: var(--bg);
      color: var(--ink);
      font: 14px/1.45 system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }}
    header {{
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 16px;
      padding: 18px 22px;
      border-bottom: 1px solid var(--line);
      background: var(--panel);
    }}
    h1 {{
      margin: 0;
      font-size: 18px;
      font-weight: 650;
      letter-spacing: 0;
    }}
    .meta {{
      margin-top: 4px;
      color: var(--muted);
      font-size: 13px;
    }}
    form {{
      display: flex;
      align-items: center;
      gap: 8px;
      flex-wrap: wrap;
    }}
    select,
    a {{
      min-height: 34px;
      border: 1px solid var(--line);
      border-radius: 6px;
      background: #fff;
      color: var(--ink);
      padding: 6px 10px;
      text-decoration: none;
      font: inherit;
    }}
    a.primary {{
      border-color: var(--accent);
      color: var(--accent);
      font-weight: 600;
    }}
    main {{
      padding: 18px;
    }}
    .graph {{
      width: 100%;
      min-height: calc(100vh - 118px);
      border: 1px solid var(--line);
      border-radius: 8px;
      background: #fff;
      overflow: auto;
    }}
    .graph img {{
      display: block;
      min-width: 760px;
      max-width: none;
      width: 100%;
      height: auto;
    }}
    .error {{
      margin: 0 0 12px;
      color: var(--danger);
      font-weight: 600;
    }}
    @media (max-width: 720px) {{
      header {{
        align-items: stretch;
        flex-direction: column;
      }}
      main {{
        padding: 10px;
      }}
      .graph {{
        min-height: calc(100vh - 178px);
      }}
    }}
  </style>
</head>
<body>
  <header>
    <div>
      <h1>libp2p / openraft graph</h1>
      <div class="meta">local peer_id: {} | refresh: 5s</div>
    </div>
    <form method="get" action="/graph">
      <select name="group_id" aria-label="Raft group" onchange="this.form.submit()">{}</select>
      <a class="primary" href="/graph.svg?group_id={}">SVG</a>
      <a href="/graph.dot?group_id={}">DOT</a>
      <a href="/cluster?group_id={}">JSON</a>
    </form>
  </header>
  <main>
    {}
    <div class="graph">
      <img src="/graph.svg?group_id={}" alt="libp2p and openraft topology">
    </div>
  </main>
</body>
</html>"#,
    html_escape(&snapshot.self_peer_id),
    group_options,
    url_escape(selected_param),
    url_escape(selected_param),
    url_escape(selected_param),
    status,
    url_escape(selected_param),
  )
}

fn html_escape(value: &str) -> String {
  value
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
}

fn url_escape(value: &str) -> String {
  value
    .bytes()
    .flat_map(|byte| match byte {
      b'A' ..= b'Z' | b'a' ..= b'z' | b'0' ..= b'9' | b'-' | b'_' | b'.' | b'~' => {
        vec![byte as char]
      }
      _ => format!("%{byte:02X}").chars().collect(),
    })
    .collect()
}

async fn openraft_nodes(
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
struct Libp2pInfoResponse {
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

async fn libp2p_info(State(state): State<Arc<AppState>>) -> Json<Libp2pInfoResponse> {
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

async fn libp2p_nodes(
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

async fn remove_openraft_member(
  State(state): State<Arc<AppState>>,
  Json(req): Json<RemoveOpenRaftMemberRequest>,
) -> Json<RemoveOpenRaftMemberResponse> {
  let group_ids = match req.group_id.clone() {
    Some(group_id) => vec![group_id],
    None => openraft_group_ids(&state.registry),
  };

  if group_ids.is_empty() {
    return Json(RemoveOpenRaftMemberResponse {
      ok: false,
      target_node_id: req.node_id,
      groups: Vec::new(),
      error: Some("openraft groups are not initialized".to_string()),
    });
  }

  let mut groups = Vec::with_capacity(group_ids.len());
  for group_id in group_ids {
    groups.push(remove_openraft_member_from_group(&state.registry, &group_id, &req.node_id).await);
  }
  let ok = groups.iter().all(|group| group.ok);
  let error = if ok {
    None
  } else {
    Some("one or more openraft membership changes failed".to_string())
  };

  Json(RemoveOpenRaftMemberResponse {
    ok,
    target_node_id: req.node_id,
    groups,
    error,
  })
}

async fn add_openraft_member(
  State(state): State<Arc<AppState>>,
  Json(req): Json<AddOpenRaftMemberRequest>,
) -> Json<AddOpenRaftMemberResponse> {
  let group_ids = match req.group_id.clone() {
    Some(group_id) => vec![group_id],
    None => openraft_group_ids(&state.registry),
  };

  if group_ids.is_empty() {
    return Json(AddOpenRaftMemberResponse {
      ok: false,
      target_node_id: req.node_id,
      groups: Vec::new(),
      error: Some("openraft groups are not initialized".to_string()),
    });
  }

  let target_addr = match resolve_openraft_member_addr(state.as_ref(), &req.node_id, req.addr).await
  {
    Ok(addr) => addr,
    Err(err) => {
      return Json(AddOpenRaftMemberResponse {
        ok: false,
        target_node_id: req.node_id,
        groups: Vec::new(),
        error: Some(err),
      });
    }
  };

  if let Err(err) = state
    .network
    .register_node(req.node_id.clone(), &target_addr)
    .await
  {
    return Json(AddOpenRaftMemberResponse {
      ok: false,
      target_node_id: req.node_id,
      groups: Vec::new(),
      error: Some(format!("register target node failed: {err}")),
    });
  }

  let catch_up_timeout = Duration::from_secs(req.catch_up_timeout_secs.unwrap_or(30));
  let mut groups = Vec::with_capacity(group_ids.len());
  for group_id in group_ids {
    groups.push(
      add_openraft_member_to_group(
        &state.registry,
        &group_id,
        &req.node_id,
        &target_addr,
        req.promote,
        catch_up_timeout,
        &state.network,
      )
      .await,
    );
  }
  let ok = groups.iter().all(|group| group.ok);
  let error = if ok {
    None
  } else {
    Some("one or more openraft membership changes failed".to_string())
  };

  Json(AddOpenRaftMemberResponse {
    ok,
    target_node_id: req.node_id,
    groups,
    error,
  })
}

async fn resolve_openraft_member_addr(
  state: &AppState,
  node_id: &NodeId,
  requested_addr: Option<String>,
) -> Result<String, String> {
  if let Some(addr) = requested_addr {
    let (peer_id, _) =
      parse_p2p_addr(&addr).map_err(|err| format!("invalid target addr: {err}"))?;
    if node_id.as_str() != peer_id.to_string() {
      return Err(format!(
        "target node_id must match addr /p2p peer id: node_id={node_id}, peer={peer_id}"
      ));
    }
    return Ok(addr);
  }

  state
    .network
    .known_nodes()
    .await
    .into_iter()
    .find(|(known_node_id, _, _)| known_node_id == node_id)
    .map(|(_, _, addr)| addr.to_string())
    .ok_or_else(|| "target addr is required for an unknown libp2p node".to_string())
}

async fn add_openraft_member_to_group(
  registry: &crate::GroupRegistry,
  group_id: &str,
  target_node_id: &NodeId,
  target_addr: &str,
  promote: bool,
  catch_up_timeout: Duration,
  network: &Libp2pNetworkFactory,
) -> AddOpenRaftMemberGroupResponse {
  let Some(group) = registry.get(group_id) else {
    return AddOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: Vec::new(),
      after_voters: Vec::new(),
      leader_id: None,
      learner_added: false,
      promoted: false,
      error: Some(format!("unknown group_id={group_id}")),
    };
  };

  let metrics = group.raft.metrics().borrow_watched().clone();
  let membership = metrics.membership_config.membership();
  let before_voters = membership.voter_ids().collect::<BTreeSet<_>>();

  if !metrics.state.is_leader() {
    // Each raft group elects its own leader, so no single node is guaranteed
    // to lead every group. For learner-only additions, forward the request to
    // this group's leader instead of rejecting it.
    if !promote && let Some(leader_id) = metrics.current_leader.clone() {
      let leader_addr = membership
        .get_node(&leader_id)
        .map(|node| node.addr.clone());
      return forward_add_learner_to_leader(
        group_id,
        target_node_id,
        target_addr,
        leader_id,
        leader_addr,
        &before_voters,
        network,
      )
      .await;
    }

    return AddOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: before_voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      learner_added: false,
      promoted: false,
      error: Some("membership changes must be submitted to the leader node".to_string()),
    };
  }

  if before_voters.contains(target_node_id) {
    return AddOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: true,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: before_voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      learner_added: false,
      promoted: false,
      error: None,
    };
  }

  let node = BasicNode {
    addr: target_addr.to_string(),
  };

  let learner_log_index = match group
    .raft
    .add_learner(target_node_id.clone(), node, false)
    .await
  {
    Ok(response) => response.log_id.index(),
    Err(err) => {
      return AddOpenRaftMemberGroupResponse {
        group_id: group_id.to_string(),
        ok: false,
        before_voters: before_voters.iter().cloned().collect(),
        after_voters: before_voters.iter().cloned().collect(),
        leader_id: metrics.current_leader,
        learner_added: false,
        promoted: false,
        error: Some(format!("add_learner failed: {err:?}")),
      };
    }
  };

  if !promote {
    let metrics = group.raft.metrics().borrow_watched().clone();
    let voters = metrics
      .membership_config
      .membership()
      .voter_ids()
      .collect::<BTreeSet<_>>();
    return AddOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: true,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      learner_added: true,
      promoted: false,
      error: None,
    };
  }

  if let Err(err) = wait_for_openraft_member_rpc(
    registry,
    group_id,
    target_node_id,
    learner_log_index,
    catch_up_timeout,
  )
  .await
  {
    let metrics = group.raft.metrics().borrow_watched().clone();
    let voters = metrics
      .membership_config
      .membership()
      .voter_ids()
      .collect::<BTreeSet<_>>();
    return AddOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      learner_added: true,
      promoted: false,
      error: Some(err),
    };
  }

  let voters = group
    .raft
    .metrics()
    .borrow_watched()
    .membership_config
    .membership()
    .voter_ids()
    .chain(std::iter::once(target_node_id.clone()))
    .collect::<BTreeSet<_>>();

  match group.raft.change_membership(voters.clone(), false).await {
    Ok(response) => {
      tracing::info!(
        group = group_id,
        target_node_id = %target_node_id,
        response = ?response,
        "added openraft voter to membership"
      );
      AddOpenRaftMemberGroupResponse {
        group_id: group_id.to_string(),
        ok: true,
        before_voters: before_voters.iter().cloned().collect(),
        after_voters: voters.iter().cloned().collect(),
        leader_id: metrics.current_leader,
        learner_added: true,
        promoted: true,
        error: None,
      }
    }
    Err(err) => AddOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      learner_added: true,
      promoted: false,
      error: Some(format!("change_membership failed: {err:?}")),
    },
  }
}

/// Forward a learner-only membership addition to the given group leader over
/// libp2p. Used when the HTTP request landed on a node that is not the leader
/// of this particular raft group.
async fn forward_add_learner_to_leader(
  group_id: &str,
  target_node_id: &NodeId,
  target_addr: &str,
  leader_id: NodeId,
  leader_addr: Option<String>,
  before_voters: &BTreeSet<NodeId>,
  network: &Libp2pNetworkFactory,
) -> AddOpenRaftMemberGroupResponse {
  if let Some(addr) = leader_addr.as_ref() {
    let _ = network.register_node(leader_id.clone(), addr).await;
  }

  let response = network
    .request(
      leader_id.clone(),
      RaftRpcRequest {
        group_id: group_id.to_string(),
        op: RaftRpcOp::AddLearner(AddLearnerRequest {
          node_id: target_node_id.clone(),
          addr: target_addr.to_string(),
        }),
      },
    )
    .await;

  let (ok, learner_added, error) = match response {
    Ok(RaftRpcResponse::AddLearner(resp)) if resp.ok => (true, true, None),
    Ok(RaftRpcResponse::AddLearner(resp)) => (
      false,
      false,
      Some(
        resp
          .error
          .unwrap_or_else(|| "add_learner forwarding rejected".to_string()),
      ),
    ),
    Ok(RaftRpcResponse::Error(message)) => (
      false,
      false,
      Some(format!("forward add_learner to leader failed: {message}")),
    ),
    Ok(other) => (
      false,
      false,
      Some(format!(
        "unexpected add_learner forwarding response: {other:?}"
      )),
    ),
    Err(err) => (
      false,
      false,
      Some(format!("forward add_learner to leader failed: {err}")),
    ),
  };

  let voters: Vec<NodeId> = before_voters.iter().cloned().collect();
  AddOpenRaftMemberGroupResponse {
    group_id: group_id.to_string(),
    ok,
    before_voters: voters.clone(),
    after_voters: voters,
    leader_id: Some(leader_id),
    learner_added,
    promoted: false,
    error,
  }
}

async fn wait_for_openraft_member_rpc(
  registry: &crate::GroupRegistry,
  group_id: &str,
  target_node_id: &NodeId,
  min_matched_index: u64,
  timeout: Duration,
) -> Result<(), String> {
  let deadline = tokio::time::Instant::now() + timeout;
  loop {
    let Some(group) = registry.get(group_id) else {
      return Err(format!("unknown group_id={group_id}"));
    };

    let metrics = group.raft.metrics().borrow_watched().clone();
    if !metrics.state.is_leader() {
      return Err("local node is no longer the leader".to_string());
    }

    let leader_last_log_index = metrics.last_log_index.unwrap_or(0);
    let target_index = metrics
      .replication
      .as_ref()
      .and_then(|replication| replication.get(target_node_id))
      .and_then(|matched| matched.as_ref())
      .map(RaftLogId::index)
      .unwrap_or(0);

    let required_index = leader_last_log_index.max(min_matched_index);
    if target_index >= required_index {
      return Ok(());
    }

    if tokio::time::Instant::now() >= deadline {
      return Err(format!(
        "learner did not catch up before timeout: matched_index={target_index}, \
         required_index={required_index}"
      ));
    }

    tracing::debug!(
      group = group_id,
      target_node_id = %target_node_id,
      matched_index = target_index,
      required_index,
      "waiting for learner to catch up"
    );
    tokio::time::sleep(Duration::from_millis(500)).await;
  }
}

async fn remove_openraft_member_from_group(
  registry: &crate::GroupRegistry,
  group_id: &str,
  target_node_id: &NodeId,
) -> RemoveOpenRaftMemberGroupResponse {
  let Some(group) = registry.get(group_id) else {
    return RemoveOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: Vec::new(),
      after_voters: Vec::new(),
      leader_id: None,
      error: Some(format!("unknown group_id={group_id}")),
    };
  };

  let metrics = group.raft.metrics().borrow_watched().clone();
  let membership = metrics.membership_config.membership();
  let before_voters = membership.voter_ids().collect::<BTreeSet<_>>();
  let after_voters = before_voters
    .iter()
    .filter(|node_id| *node_id != target_node_id)
    .cloned()
    .collect::<BTreeSet<_>>();

  if !metrics.state.is_leader() {
    return RemoveOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: after_voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      error: Some("membership changes must be submitted to the leader node".to_string()),
    };
  }

  if !before_voters.contains(target_node_id) {
    return RemoveOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: true,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: before_voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      error: None,
    };
  }

  if after_voters.is_empty() {
    return RemoveOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: Vec::new(),
      leader_id: metrics.current_leader,
      error: Some("refusing to remove the last openraft voter".to_string()),
    };
  }

  match group
    .raft
    .change_membership(after_voters.clone(), false)
    .await
  {
    Ok(response) => {
      tracing::info!(
        group = group_id,
        target_node_id = %target_node_id,
        response = ?response,
        "removed openraft voter from membership"
      );
      RemoveOpenRaftMemberGroupResponse {
        group_id: group_id.to_string(),
        ok: true,
        before_voters: before_voters.iter().cloned().collect(),
        after_voters: after_voters.iter().cloned().collect(),
        leader_id: metrics.current_leader,
        error: None,
      }
    }
    Err(err) => RemoveOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: after_voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      error: Some(format!("change_membership failed: {err:?}")),
    },
  }
}

fn openraft_group_ids(registry: &crate::GroupRegistry) -> Vec<String> {
  registry
    .all()
    .map(|groups| groups.keys().cloned().collect())
    .unwrap_or_default()
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

async fn cluster_info(
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

fn unix_now_secs() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs()
}

/// Submit a task state-machine command via the node's task frontend:
/// control nodes write through their local raft handle (following a leader
/// hint over the network when needed), workers go through the tarpc TaskRpc.
async fn submit_task_state_command(
  state: &AppState,
  cmd: StateCommand,
) -> anyhow::Result<TaskOpResult> {
  let group_id = groups::TASKS.to_string();
  match &state.task_frontend {
    TaskFrontend::Control => {
      let reply = TaskRpcService
        .submit(
          tarpc::context::current(),
          group_id.clone(),
          cmd.clone().into(),
        )
        .await;
      if reply.ok {
        let value = reply
          .value
          .ok_or_else(|| anyhow::anyhow!("task command accepted but returned no result"))?;
        return sonic_rs::from_str(&value)
          .map_err(|err| anyhow::anyhow!("decode task op result: {err}"));
      }
      // Not the leader for this group: follow the hint over the network.
      if let Some(leader_id) = reply.leader_id.as_deref() {
        let leader = NodeId::new(leader_id);
        if let Some(addr) = reply.leader_addr.as_deref() {
          let _ = state.network.register_node(leader.clone(), addr).await;
        }
        let control_nodes = tokio::sync::Mutex::new(ControlNodes::new(vec![leader]));
        return submit_command(&state.network, &control_nodes, &group_id, cmd).await;
      }
      Err(anyhow::anyhow!(
        "task command failed: {}",
        reply.error.unwrap_or_default()
      ))
    }
    TaskFrontend::Worker { control_nodes } => {
      submit_command(&state.network, control_nodes, &group_id, cmd).await
    }
  }
}

/// Enqueue one task with an already-encoded (kind-tagged) payload.
async fn enqueue_task(
  state: &AppState,
  payload: String,
  idem_key: Option<String>,
  delay_secs: u64,
) -> EmailResponse {
  if payload.len() > crate::tasks::MAX_TASK_PAYLOAD_BYTES {
    return push_error(format!(
      "task payload is {} bytes, over the {} byte limit (wasm modules must fit the raft log)",
      payload.len(),
      crate::tasks::MAX_TASK_PAYLOAD_BYTES
    ));
  }
  let now = unix_now_secs();
  let cmd = StateCommand::TaskEnqueue {
    id: uuid::Uuid::now_v7().to_string(),
    payload,
    run_at: now + delay_secs,
    idem_key,
    created_at: now,
  };

  match submit_task_state_command(state, cmd).await {
    Ok(result) if result.ok => EmailResponse {
      ok: true,
      task_id: result.id,
      deduplicated: result.deduplicated,
      error: None,
    },
    Ok(result) => EmailResponse {
      ok: false,
      task_id: result.id,
      deduplicated: result.deduplicated,
      error: result.reason,
    },
    Err(err) => EmailResponse {
      ok: false,
      task_id: None,
      deduplicated: None,
      error: Some(err.to_string()),
    },
  }
}

fn push_error(message: String) -> EmailResponse {
  EmailResponse {
    ok: false,
    task_id: None,
    deduplicated: None,
    error: Some(message),
  }
}

async fn push_email(
  State(state): State<Arc<AppState>>,
  Json(req): Json<EmailRequest>,
) -> Json<EmailResponse> {
  let payload = match TaskPayload::Email(Email { to: req.to }).encode() {
    Ok(payload) => payload,
    Err(err) => return Json(push_error(err)),
  };
  Json(enqueue_task(&state, payload, req.idem_key, 0).await)
}

/// Generic multi-kind submission. The payload must decode into a
/// [`TaskPayload`] variant, so an unknown or malformed kind is rejected here
/// at submit time (not at execution).
async fn push_task(
  State(state): State<Arc<AppState>>,
  Json(req): Json<PushTaskRequest>,
) -> Json<EmailResponse> {
  let payload = match sonic_rs::to_string(&req.payload) {
    Ok(payload) => payload,
    Err(err) => return Json(push_error(format!("encode task payload: {err}"))),
  };
  if let Err(err) = TaskPayload::decode(&payload) {
    return Json(push_error(err));
  }
  Json(enqueue_task(&state, payload, req.idem_key, req.delay_secs).await)
}

async fn list_tasks(State(state): State<Arc<AppState>>) -> Json<TasksResponse> {
  let group_id = groups::TASKS.to_string();
  let reply = match &state.task_frontend {
    TaskFrontend::Control => {
      TaskRpcService
        .list_tasks(tarpc::context::current(), group_id)
        .await
    }
    TaskFrontend::Worker { control_nodes } => {
      match call_read(&state.network, control_nodes, || {
        TaskRpcRequest::ListTasks {
          group_id: group_id.clone(),
        }
      })
      .await
      {
        Ok(TaskRpcResponse::ListTasks(reply)) => reply,
        Ok(other) => {
          return Json(TasksResponse {
            ok: false,
            tasks: Vec::new(),
            error: Some(format!("unexpected task rpc response: {other:?}")),
          });
        }
        Err(err) => {
          return Json(TasksResponse {
            ok: false,
            tasks: Vec::new(),
            error: Some(err.to_string()),
          });
        }
      }
    }
  };

  Json(TasksResponse {
    ok: reply.ok,
    tasks: reply.tasks,
    error: reply.error,
  })
}

async fn list_task_workers(State(state): State<Arc<AppState>>) -> Json<TaskWorkersResponse> {
  let group_id = groups::TASKS.to_string();
  let reply = match &state.task_frontend {
    TaskFrontend::Control => {
      TaskRpcService
        .list_workers(tarpc::context::current(), group_id)
        .await
    }
    TaskFrontend::Worker { control_nodes } => {
      match call_read(&state.network, control_nodes, || {
        TaskRpcRequest::ListWorkers {
          group_id: group_id.clone(),
        }
      })
      .await
      {
        Ok(TaskRpcResponse::ListWorkers(reply)) => reply,
        Ok(other) => {
          return Json(TaskWorkersResponse {
            ok: false,
            workers: Vec::new(),
            error: Some(format!("unexpected task rpc response: {other:?}")),
          });
        }
        Err(err) => {
          return Json(TaskWorkersResponse {
            ok: false,
            workers: Vec::new(),
            error: Some(err.to_string()),
          });
        }
      }
    }
  };

  Json(TaskWorkersResponse {
    ok: reply.ok,
    workers: reply.workers,
    error: reply.error,
  })
}

async fn task_metrics(State(state): State<Arc<AppState>>) -> Json<TaskMetricsResponse> {
  let group_id = groups::TASKS.to_string();
  let reply = match &state.task_frontend {
    TaskFrontend::Control => {
      TaskRpcService
        .metrics(tarpc::context::current(), group_id)
        .await
    }
    TaskFrontend::Worker { control_nodes } => {
      match call_read(&state.network, control_nodes, || TaskRpcRequest::Metrics {
        group_id: group_id.clone(),
      })
      .await
      {
        Ok(TaskRpcResponse::Metrics(reply)) => reply,
        Ok(other) => {
          return Json(TaskMetricsResponse {
            ok: false,
            metrics: None,
            error: Some(format!("unexpected task rpc response: {other:?}")),
          });
        }
        Err(err) => {
          return Json(TaskMetricsResponse {
            ok: false,
            metrics: None,
            error: Some(err.to_string()),
          });
        }
      }
    }
  };

  Json(TaskMetricsResponse {
    ok: reply.ok,
    metrics: reply.metrics,
    error: reply.error,
  })
}

async fn set_value(
  State(state): State<Arc<AppState>>,
  Json(req): Json<WriteValueRequest>,
) -> Json<WriteValueResponse> {
  let group_id = match resolve_group_id(state.as_ref(), req.group_id) {
    Ok(group_id) => group_id,
    Err(err) => {
      return Json(WriteValueResponse {
        target_node_id: None,
        ok: false,
        value: None,
        error: Some(err),
      });
    }
  };

  let request = RaftKvRequest {
    group_id: group_id.clone(),
    op: Some(KvRequestOp::Set(SetValueRequest {
      key: req.key,
      value: req.value,
    })),
  };
  let (target_node_id, response) =
    match send_kv_request(state.as_ref(), &group_id, req.target_node_id, request).await {
      Ok((id, resp)) => (Some(id), resp),
      Err(err) => {
        return Json(WriteValueResponse {
          target_node_id: None,
          ok: false,
          value: None,
          error: Some(err),
        });
      }
    };

  match response.op {
    Some(KvResponseOp::Set(resp)) => Json(WriteValueResponse {
      target_node_id,
      ok: resp.ok,
      value: Some(resp.value),
      error: None,
    }),
    Some(KvResponseOp::Error(err)) => Json(WriteValueResponse {
      target_node_id,
      ok: false,
      value: None,
      error: Some(err.message),
    }),
    other => Json(WriteValueResponse {
      target_node_id,
      ok: false,
      value: None,
      error: Some(format!("unexpected response: {other:?}")),
    }),
  }
}

async fn send_chat(
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

async fn sync_snapshot(
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

async fn update_value(
  State(state): State<Arc<AppState>>,
  Json(req): Json<UpdateValueRequest>,
) -> Json<UpdateValueResponse> {
  let group_id = match resolve_group_id(state.as_ref(), req.group_id) {
    Ok(group_id) => group_id,
    Err(err) => {
      return Json(UpdateValueResponse {
        target_node_id: None,
        ok: false,
        value: None,
        error: Some(err),
      });
    }
  };

  let request = RaftKvRequest {
    group_id: group_id.clone(),
    op: Some(KvRequestOp::Update(ProtoUpdateValueRequest {
      key: req.key,
      value: req.value,
    })),
  };
  let (target_node_id, response) =
    match send_kv_request(state.as_ref(), &group_id, req.target_node_id, request).await {
      Ok((id, resp)) => (Some(id), resp),
      Err(err) => {
        return Json(UpdateValueResponse {
          target_node_id: None,
          ok: false,
          value: None,
          error: Some(err),
        });
      }
    };

  match response.op {
    Some(KvResponseOp::Update(resp)) => Json(UpdateValueResponse {
      target_node_id,
      ok: resp.ok,
      value: Some(resp.value),
      error: None,
    }),
    Some(KvResponseOp::Error(err)) => Json(UpdateValueResponse {
      target_node_id,
      ok: false,
      value: None,
      error: Some(err.message),
    }),
    other => Json(UpdateValueResponse {
      target_node_id,
      ok: false,
      value: None,
      error: Some(format!("unexpected response: {other:?}")),
    }),
  }
}

async fn delete_value(
  State(state): State<Arc<AppState>>,
  Json(req): Json<DeleteValueRequestBody>,
) -> Json<DeleteValueResponseBody> {
  let group_id = match resolve_group_id(state.as_ref(), req.group_id) {
    Ok(group_id) => group_id,
    Err(err) => {
      return Json(DeleteValueResponseBody {
        target_node_id: None,
        ok: false,
        error: Some(err),
      });
    }
  };

  let request = RaftKvRequest {
    group_id: group_id.clone(),
    op: Some(KvRequestOp::Delete(DeleteValueRequest { key: req.key })),
  };
  let (target_node_id, response) =
    match send_kv_request(state.as_ref(), &group_id, req.target_node_id, request).await {
      Ok((id, resp)) => (Some(id), resp),
      Err(err) => {
        return Json(DeleteValueResponseBody {
          target_node_id: None,
          ok: false,
          error: Some(err),
        });
      }
    };

  match response.op {
    Some(KvResponseOp::Delete(resp)) => Json(DeleteValueResponseBody {
      target_node_id,
      ok: resp.ok,
      error: None,
    }),
    Some(KvResponseOp::Error(err)) => Json(DeleteValueResponseBody {
      target_node_id,
      ok: false,
      error: Some(err.message),
    }),
    other => Json(DeleteValueResponseBody {
      target_node_id,
      ok: false,
      error: Some(format!("unexpected response: {other:?}")),
    }),
  }
}

async fn write_cached_value(
  State(state): State<Arc<AppState>>,
  Json(req): Json<CacheWriteRequest>,
) -> Json<CacheWriteResponse> {
  let Some(cache) = state.sqlite_cache.as_ref() else {
    return Json(CacheWriteResponse {
      target_node_id: None,
      ok: false,
      pending_key: None,
      error: Some("sqlite cache is disabled".to_string()),
    });
  };

  let group_id = match resolve_group_id(state.as_ref(), req.group_id) {
    Ok(group_id) => group_id,
    Err(err) => {
      return Json(CacheWriteResponse {
        target_node_id: None,
        ok: false,
        pending_key: None,
        error: Some(err),
      });
    }
  };

  if let Err(err) = cache.write_redis(&req.key, &req.value).await {
    return Json(CacheWriteResponse {
      target_node_id: None,
      ok: false,
      pending_key: None,
      error: Some(err.to_string()),
    });
  }

  let openraft_key = pending_key(&req.key);
  let group = match state.registry.get(&group_id) {
    Some(group) => group,
    None => {
      return Json(CacheWriteResponse {
        target_node_id: None,
        ok: false,
        pending_key: Some(openraft_key),
        error: Some(format!("unknown group_id={group_id}")),
      });
    }
  };

  if req.target_node_id.is_some() {
    let request = RaftKvRequest {
      group_id: group_id.clone(),
      op: Some(KvRequestOp::Set(SetValueRequest {
        key: openraft_key.clone(),
        value: "1".to_string(),
      })),
    };
    let (target_node_id, response) =
      match send_kv_request(state.as_ref(), &group_id, req.target_node_id, request).await {
        Ok((id, resp)) => (Some(id), resp),
        Err(err) => {
          return Json(CacheWriteResponse {
            target_node_id: None,
            ok: false,
            pending_key: Some(openraft_key),
            error: Some(err),
          });
        }
      };

    return match response.op {
      Some(KvResponseOp::Set(resp)) if resp.ok => Json(CacheWriteResponse {
        target_node_id,
        ok: true,
        pending_key: Some(openraft_key),
        error: None,
      }),
      Some(KvResponseOp::Error(err)) => Json(CacheWriteResponse {
        target_node_id,
        ok: false,
        pending_key: Some(openraft_key),
        error: Some(err.message),
      }),
      other => Json(CacheWriteResponse {
        target_node_id,
        ok: false,
        pending_key: Some(openraft_key),
        error: Some(format!("unexpected response: {other:?}")),
      }),
    };
  }

  match record_pending_key(group_id, &group, &state.kv_client, &req.key).await {
    Ok(target_node_id) => Json(CacheWriteResponse {
      target_node_id: Some(target_node_id),
      ok: true,
      pending_key: Some(openraft_key),
      error: None,
    }),
    Err(err) => Json(CacheWriteResponse {
      target_node_id: None,
      ok: false,
      pending_key: Some(openraft_key),
      error: Some(err.to_string()),
    }),
  }
}

async fn read_cached_value(
  State(state): State<Arc<AppState>>,
  Json(req): Json<CacheReadRequest>,
) -> Json<CacheReadResponse> {
  let Some(cache) = state.sqlite_cache.as_ref() else {
    return Json(CacheReadResponse {
      ok: false,
      found: false,
      value: None,
      error: Some("sqlite cache is disabled".to_string()),
    });
  };

  match cache.read_cached(&req.key).await {
    Ok(Some(value)) => Json(CacheReadResponse {
      ok: true,
      found: true,
      value: Some(value),
      error: None,
    }),
    Ok(None) => Json(CacheReadResponse {
      ok: true,
      found: false,
      value: None,
      error: None,
    }),
    Err(err) => Json(CacheReadResponse {
      ok: false,
      found: false,
      value: None,
      error: Some(err.to_string()),
    }),
  }
}

async fn list_sqlite_values(State(state): State<Arc<AppState>>) -> Json<SqliteValuesResponse> {
  let Some(cache) = state.sqlite_cache.as_ref() else {
    return Json(SqliteValuesResponse {
      ok: false,
      values: Vec::new(),
      error: Some("sqlite cache is disabled".to_string()),
    });
  };

  match cache.list_sqlite_values().await {
    Ok(values) => Json(SqliteValuesResponse {
      ok: true,
      values,
      error: None,
    }),
    Err(err) => Json(SqliteValuesResponse {
      ok: false,
      values: Vec::new(),
      error: Some(err.to_string()),
    }),
  }
}

/// Prometheus exposition endpoint (`GET /metrics`).
async fn prometheus_metrics() -> String {
  crate::telemetry::prometheus_handle().render()
}

#[derive(serde::Serialize)]
struct GroupsResponse {
  ok: bool,
  /// Live raft groups served by this node (from the group registry, not the
  /// static startup list).
  groups: Vec<String>,
  default_group: GroupId,
  initialized: bool,
}

/// `GET /groups`: discover the raft groups this node actually serves.
async fn list_groups(State(state): State<Arc<AppState>>) -> Json<GroupsResponse> {
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
struct RuntimeConfigResponse {
  ok: bool,
  config: crate::runtime_config::RuntimeConfig,
  error: Option<String>,
}

/// `GET /config`: current hot-reloadable runtime configuration.
async fn get_runtime_config() -> Json<RuntimeConfigResponse> {
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
async fn update_runtime_config(
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

async fn send_kv_request(
  state: &AppState,
  group_id: &str,
  target_node_id: Option<NodeId>,
  request: RaftKvRequest,
) -> Result<(NodeId, RaftKvResponse), String> {
  match resolve_kv_target(state, group_id, target_node_id).await? {
    KvTarget::Local { node_id } => {
      let group = state
        .registry
        .get(group_id)
        .ok_or_else(|| format!("unknown group_id={group_id}"))?;
      let resp = process_kv_request(group.raft, group.kv_data, request).await;
      Ok((node_id, resp))
    }
    KvTarget::Remote {
      node_id,
      peer,
      addr,
    } => {
      state
        .kv_client
        .connect(peer, addr)
        .await
        .map_err(|err| format!("libp2p connect error: {err}"))?;
      let resp = state
        .kv_client
        .request(peer, request)
        .await
        .map_err(|err| format!("libp2p error: {err}"))?;
      Ok((node_id, resp))
    }
  }
}

enum KvTarget {
  Local {
    node_id: NodeId,
  },
  Remote {
    node_id: NodeId,
    peer: PeerId,
    addr: Multiaddr,
  },
}

fn resolve_group_id(state: &AppState, group_id: Option<String>) -> Result<GroupId, String> {
  match group_id {
    Some(group_id) => {
      if state
        .registry
        .all()
        .is_some_and(|groups| groups.contains_key(&group_id))
      {
        Ok(group_id)
      } else {
        Err(format!("unknown group_id={group_id}"))
      }
    }
    None => Ok(state.default_group.clone()),
  }
}

async fn resolve_kv_target(
  state: &AppState,
  group_id: &str,
  target_node_id: Option<NodeId>,
) -> Result<KvTarget, String> {
  let group = state
    .registry
    .get(group_id)
    .ok_or_else(|| format!("unknown group_id={group_id}"))?;
  let metrics = group.raft.metrics().borrow_watched().clone();
  let candidate = target_node_id.or_else(|| metrics.current_leader.clone());

  if metrics.state.is_leader() || candidate.as_ref() == Some(&state.node_id) {
    return Ok(KvTarget::Local {
      node_id: state.node_id.clone(),
    });
  }

  let nodes = state.network.known_nodes().await;
  if nodes.is_empty() {
    return Ok(KvTarget::Local {
      node_id: state.node_id.clone(),
    });
  }

  let node_id = candidate
    .filter(|id| id != &state.node_id)
    .or_else(|| {
      nodes
        .iter()
        .find(|(id, _, _)| id != &state.node_id)
        .map(|(id, _, _)| id.clone())
    })
    .or_else(|| nodes.first().map(|(id, _, _)| id.clone()))
    .ok_or_else(|| "no leader available".to_string())?;

  nodes
    .into_iter()
    .find(|(id, _, _)| id == &node_id)
    .map(|(id, peer, addr)| KvTarget::Remote {
      node_id: id,
      peer,
      addr,
    })
    .ok_or_else(|| format!("unknown target node_id={node_id}"))
}

fn string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
  D: Deserializer<'de>,
{
  struct StringOrNumberVisitor;

  impl Visitor<'_> for StringOrNumberVisitor {
    type Value = String;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      formatter.write_str("a string, number, or bool")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(value.to_owned())
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(value)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(value.to_string())
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(value.to_string())
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(value.to_string())
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(value.to_string())
    }
  }

  deserializer.deserialize_any(StringOrNumberVisitor)
}

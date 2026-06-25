use std::{
  collections::{BTreeMap, BTreeSet},
  net::SocketAddr,
  sync::Arc,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use apalis::prelude::TaskSink;
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
  apalis_raft::{Email, RaftApalisStorage, TaskRecordView, WorkerRecord},
  graphviz::{ClusterGraphNode, ClusterGraphSnapshot, cluster_graph_dot, cluster_graph_svg},
  network::{
    openraft_dispatcher::process_kv_request,
    rpc::{RaftRpcOp, RaftRpcRequest, RaftRpcResponse},
    swarm::{GOSSIP_TOPIC, KvClient},
    transport::{Libp2pNetworkFactory, parse_p2p_addr},
  },
  openraft_group, openraft_groups,
  proto::raft_kv::{
    ChatMessage, DeleteValueRequest, RaftKvRequest, RaftKvResponse, SetValueRequest,
    UpdateValueRequest as ProtoUpdateValueRequest, raft_kv_request::Op as KvRequestOp,
    raft_kv_response::Op as KvResponseOp,
  },
  signal::ShutdownRx,
  sqlite_cache::{CachedValue, SqliteCache, pending_key, record_pending_key},
  store::ensure_linearizable_read,
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
  pub network: Libp2pNetworkFactory,
  pub kv_client: KvClient,
  pub default_group: GroupId,
  pub apalis_email: Option<RaftApalisStorage<Email>>,
  pub sqlite_cache: Option<SqliteCache>,
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
    .route("/cluster/openraft", get(openraft_nodes))
    .route("/cluster/libp2p", get(libp2p_nodes))
    .route("/openraft/membership/add", post(add_openraft_member))
    .route("/openraft/membership/remove", post(remove_openraft_member))
    .route("/graph", get(cluster_graph_page))
    .route("/graph.dot", get(cluster_graph_dot_response))
    .route("/graph.svg", get(cluster_graph_svg_response))
    .route("/chat", post(send_chat))
    .route("/sync/snapshot", post(sync_snapshot))
    .route("/apalis/email", post(push_email))
    .route("/apalis/tasks", get(list_apalis_tasks))
    .route("/apalis/workers", get(list_apalis_workers))
    .route("/write", post(set_value))
    .route("/update", post(update_value))
    .route("/delete", post(delete_value))
    .route("/cache/write", post(write_cached_value))
    .route("/cache/read", post(read_cached_value))
    .route("/sqlite/values", get(list_sqlite_values))
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
  kv_data: Vec<KvPairResponse>,
  error: Option<String>,
}

#[derive(Serialize)]
struct KnownNodeResponse {
  node_id: NodeId,
  peer_id: String,
  addr: String,
}

#[derive(Serialize)]
struct OpenRaftNodesResponse {
  ok: bool,
  group_id: String,
  groups: Vec<String>,
  local_node_id: NodeId,
  local_peer_id: String,
  leader_id: Option<NodeId>,
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
}

#[derive(Serialize)]
struct EmailResponse {
  ok: bool,
  error: Option<String>,
}

#[derive(Serialize)]
struct ApalisTasksResponse {
  ok: bool,
  tasks: Vec<TaskRecordView>,
  error: Option<String>,
}

#[derive(Serialize)]
struct ApalisWorkersResponse {
  ok: bool,
  workers: Vec<WorkerRecord>,
  error: Option<String>,
}

#[derive(Deserialize)]
struct ClusterQuery {
  #[serde(alias = "group")]
  group_id: Option<String>,
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
  let group_id = query
    .group_id
    .unwrap_or_else(|| state.default_group.clone());
  let groups = openraft_groups()
    .map(|groups| groups.keys().cloned().collect())
    .unwrap_or_default();

  let (metrics, error) = match openraft_group(&group_id) {
    Some(group) => (Some(group.raft.metrics().borrow_watched().clone()), None),
    None if openraft_groups().is_none() => (
      None,
      Some("openraft groups are not initialized".to_string()),
    ),
    None => (None, Some(format!("unknown group_id={group_id}"))),
  };

  let known_nodes = state.network.known_nodes().await;
  let mut nodes = Vec::with_capacity(known_nodes.len());
  for (node_id, peer_id, addr) in known_nodes {
    let connected = state.network.is_peer_connected(&peer_id).await;
    let server_state = if node_id == state.node_id {
      metrics.as_ref().map(|metrics| metrics.state)
    } else if connected {
      remote_server_state(&group_id, &node_id, &state.network).await
    } else {
      None
    };
    nodes.push(ClusterGraphNode {
      node_id,
      peer_id: peer_id.to_string(),
      addr: addr.to_string(),
      connected,
      server_state,
    });
  }

  ClusterGraphSnapshot {
    self_node_id: state.node_id.clone(),
    self_peer_id: state.peer_id.clone(),
    self_listen: state.listen.clone(),
    group_id,
    groups,
    nodes,
    metrics,
    error,
  }
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
    Ok(RaftRpcResponse::Error(message)) => {
      tracing::debug!(
        group = group_id,
        node_id = %node_id,
        error = %message,
        "remote openraft metrics request returned error while rendering graph"
      );
      None
    }
    Ok(other) => {
      tracing::debug!(
        group = group_id,
        node_id = %node_id,
        response = ?other,
        "unexpected remote openraft metrics response while rendering graph"
      );
      None
    }
    Err(err) => {
      tracing::debug!(
        group = group_id,
        node_id = %node_id,
        error = ?err,
        "remote openraft metrics request failed while rendering graph"
      );
      None
    }
  }
}

fn render_cluster_graph_page(snapshot: &ClusterGraphSnapshot) -> String {
  let group_options = snapshot
    .groups
    .iter()
    .map(|group| {
      let selected = if group == &snapshot.group_id {
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
    })
    .collect::<String>();
  let status = snapshot
    .error
    .as_ref()
    .map(|err| format!("<p class=\"error\">{}</p>", html_escape(err)))
    .unwrap_or_default();
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
    url_escape(&snapshot.group_id),
    url_escape(&snapshot.group_id),
    url_escape(&snapshot.group_id),
    status,
    url_escape(&snapshot.group_id),
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
  let group_id = query
    .group_id
    .unwrap_or_else(|| state.default_group.clone());
  let groups = openraft_group_ids();
  let Some(group) = openraft_group(&group_id) else {
    let error = if openraft_groups().is_none() {
      "openraft groups are not initialized".to_string()
    } else {
      format!("unknown group_id={group_id}")
    };
    return Json(OpenRaftNodesResponse {
      ok: false,
      group_id,
      groups,
      local_node_id: state.node_id.clone(),
      local_peer_id: state.peer_id.clone(),
      leader_id: None,
      raft_state: None,
      voters: 0,
      learners: 0,
      nodes: Vec::new(),
      error: Some(error),
    });
  };

  let metrics = group.raft.metrics().borrow_watched().clone();
  let membership = metrics.membership_config.membership();
  let voters = membership.voter_ids().collect::<BTreeSet<_>>();
  let learners = membership.learner_ids().collect::<BTreeSet<_>>();
  let known_nodes = known_nodes_by_id(&state.network).await;
  let mut nodes = Vec::new();

  for (node_id, node) in membership.nodes() {
    let is_leader = metrics.current_leader.as_ref() == Some(node_id);
    let role = if is_leader {
      "leader"
    } else if voters.contains(node_id) {
      "voter"
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

  Json(OpenRaftNodesResponse {
    ok: true,
    group_id,
    groups,
    local_node_id: state.node_id.clone(),
    local_peer_id: state.peer_id.clone(),
    leader_id: metrics.current_leader.clone(),
    raft_state: Some(server_state_name(metrics.state)),
    voters: voters.len(),
    learners: learners.len(),
    nodes,
    error: None,
  })
}

async fn libp2p_nodes(
  State(state): State<Arc<AppState>>,
  Query(query): Query<ClusterQuery>,
) -> Json<Libp2pNodesResponse> {
  let group_id = query
    .group_id
    .unwrap_or_else(|| state.default_group.clone());
  let (roles, error) = match openraft_roles_by_node(&group_id) {
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
  State(_state): State<Arc<AppState>>,
  Json(req): Json<RemoveOpenRaftMemberRequest>,
) -> Json<RemoveOpenRaftMemberResponse> {
  let group_ids = match req.group_id.clone() {
    Some(group_id) => vec![group_id],
    None => openraft_group_ids(),
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
    groups.push(remove_openraft_member_from_group(&group_id, &req.node_id).await);
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
    None => openraft_group_ids(),
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
        &group_id,
        &req.node_id,
        &target_addr,
        req.promote,
        catch_up_timeout,
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
  group_id: &str,
  target_node_id: &NodeId,
  target_addr: &str,
  promote: bool,
  catch_up_timeout: Duration,
) -> AddOpenRaftMemberGroupResponse {
  let Some(group) = openraft_group(group_id) else {
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

async fn wait_for_openraft_member_rpc(
  group_id: &str,
  target_node_id: &NodeId,
  min_matched_index: u64,
  timeout: Duration,
) -> Result<(), String> {
  let deadline = tokio::time::Instant::now() + timeout;
  loop {
    let Some(group) = openraft_group(group_id) else {
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
  group_id: &str,
  target_node_id: &NodeId,
) -> RemoveOpenRaftMemberGroupResponse {
  let Some(group) = openraft_group(group_id) else {
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

fn openraft_group_ids() -> Vec<String> {
  openraft_groups()
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

fn openraft_roles_by_node(group_id: &str) -> Result<BTreeMap<NodeId, String>, String> {
  let Some(group) = openraft_group(group_id) else {
    return if openraft_groups().is_none() {
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
  let mut nodes: Vec<KnownNodeResponse> = state
    .network
    .known_nodes()
    .await
    .into_iter()
    .map(|(node_id, peer_id, addr)| KnownNodeResponse {
      node_id,
      peer_id: peer_id.to_string(),
      addr: addr.to_string(),
    })
    .collect();

  nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));

  let group_id = query
    .group_id
    .unwrap_or_else(|| state.default_group.clone());

  let Some(global_groups) = openraft_groups() else {
    return Json(ClusterInfoResponse {
      node_id: state.node_id.clone(),
      node_name: state.node_name.clone(),
      peer_id: state.peer_id.clone(),
      listen: state.listen.clone(),
      group_id,
      groups: Vec::new(),
      known_nodes: nodes,
      raft_metrics: sonic_rs::Value::from_static_str("openraft groups are not initialized"),
      kv_data: Vec::new(),
      error: Some("openraft groups are not initialized".to_string()),
    });
  };

  let groups: Vec<String> = global_groups.keys().cloned().collect();

  let Some(group) = openraft_group(&group_id) else {
    return Json(ClusterInfoResponse {
      node_id: state.node_id.clone(),
      node_name: state.node_name.clone(),
      peer_id: state.peer_id.clone(),
      listen: state.listen.clone(),
      group_id,
      groups,
      known_nodes: nodes,
      raft_metrics: sonic_rs::Value::from_static_str("unknown group"),
      kv_data: Vec::new(),
      error: Some("unknown group_id".to_string()),
    });
  };

  let metrics = group.raft.metrics().borrow_watched().clone();
  let raft_metrics = sonic_rs::to_value(&metrics)
    .unwrap_or_else(|err| sonic_rs::Value::copy_str(&format!("metrics serialize error: {err}")));

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
    kv_data,
    error: None,
  })
}

async fn push_email(
  State(state): State<Arc<AppState>>,
  Json(req): Json<EmailRequest>,
) -> Json<EmailResponse> {
  let Some(mut storage) = state.apalis_email.clone() else {
    return Json(EmailResponse {
      ok: false,
      error: Some("apalis storage is not available on this node".to_string()),
    });
  };
  match storage.push(Email { to: req.to }).await {
    Ok(()) => Json(EmailResponse {
      ok: true,
      error: None,
    }),
    Err(err) => Json(EmailResponse {
      ok: false,
      error: Some(err.to_string()),
    }),
  }
}

async fn list_apalis_tasks(State(state): State<Arc<AppState>>) -> Json<ApalisTasksResponse> {
  let Some(storage) = state.apalis_email.clone() else {
    return Json(ApalisTasksResponse {
      ok: false,
      tasks: Vec::new(),
      error: Some("apalis storage is not available on this node".to_string()),
    });
  };

  match storage.list_tasks().await {
    Ok(tasks) => Json(ApalisTasksResponse {
      ok: true,
      tasks,
      error: None,
    }),
    Err(err) => Json(ApalisTasksResponse {
      ok: false,
      tasks: Vec::new(),
      error: Some(err.to_string()),
    }),
  }
}

async fn list_apalis_workers(State(state): State<Arc<AppState>>) -> Json<ApalisWorkersResponse> {
  let Some(storage) = state.apalis_email.clone() else {
    return Json(ApalisWorkersResponse {
      ok: false,
      workers: Vec::new(),
      error: Some("apalis storage is not available on this node".to_string()),
    });
  };

  match storage.list_workers().await {
    Ok(workers) => Json(ApalisWorkersResponse {
      ok: true,
      workers,
      error: None,
    }),
    Err(err) => Json(ApalisWorkersResponse {
      ok: false,
      workers: Vec::new(),
      error: Some(err.to_string()),
    }),
  }
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
  let group = match openraft_group(&group_id) {
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

async fn send_kv_request(
  state: &AppState,
  group_id: &str,
  target_node_id: Option<NodeId>,
  request: RaftKvRequest,
) -> Result<(NodeId, RaftKvResponse), String> {
  match resolve_kv_target(state, group_id, target_node_id).await? {
    KvTarget::Local { node_id } => {
      let group = openraft_group(group_id).ok_or_else(|| format!("unknown group_id={group_id}"))?;
      let resp =
        process_kv_request(group.raft, group.kv_data, state.kv_client.clone(), request).await;
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
      if openraft_groups().is_some_and(|groups| groups.contains_key(&group_id)) {
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
  let group = openraft_group(group_id).ok_or_else(|| format!("unknown group_id={group_id}"))?;
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

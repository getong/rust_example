use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use axum::{
  Router,
  body::to_bytes,
  extract::{FromRequest, Request},
  http::{
    StatusCode,
    header::{CONTENT_TYPE, HeaderMap, HeaderValue},
  },
  response::{IntoResponse, Response},
  routing::{get, post},
};
use openraft::ServerState;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
  GroupId, NodeId,
  network::{
    rpc::{RaftRpcOp, RaftRpcRequest, RaftRpcResponse},
    swarm::{KvClient, Libp2pClient},
    transport::Libp2pNetworkFactory,
  },
  signal::ShutdownRx,
  sqlite_cache::SqliteCache,
  tasks::rpc::ControlNodes,
};

pub mod cluster;
pub mod graph;
pub mod kv;
pub mod membership;
pub mod task;

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
    .route("/cluster", get(cluster::cluster_info))
    .route("/openraft/nodes", get(cluster::openraft_nodes))
    .route("/libp2p/nodes", get(cluster::libp2p_nodes))
    .route("/libp2p/info", get(cluster::libp2p_info))
    .route("/cluster/openraft", get(cluster::openraft_nodes))
    .route("/cluster/libp2p", get(cluster::libp2p_nodes))
    .route(
      "/openraft/membership/add",
      post(membership::add_openraft_member),
    )
    .route(
      "/openraft/membership/remove",
      post(membership::remove_openraft_member),
    )
    .route(
      "/openraft/membership/batch",
      post(membership::batch_openraft_membership),
    )
    .route(
      "/openraft/membership/replace",
      post(membership::replace_openraft_member),
    )
    .route("/graph", get(graph::cluster_graph_page))
    .route("/graph.dot", get(graph::cluster_graph_dot_response))
    .route("/graph.svg", get(graph::cluster_graph_svg_response))
    .route("/chat", post(cluster::send_chat))
    .route("/sync/snapshot", post(cluster::sync_snapshot))
    .route("/tasks/email", post(task::push_email))
    .route("/tasks/push", post(task::push_task))
    .route("/tasks", get(task::list_tasks))
    .route("/tasks/workers", get(task::list_task_workers))
    .route("/tasks/metrics", get(task::task_metrics))
    .route("/write", post(kv::set_value))
    .route("/update", post(kv::update_value))
    .route("/delete", post(kv::delete_value))
    .route("/cache/write", post(kv::write_cached_value))
    .route("/cache/read", post(kv::read_cached_value))
    .route("/sqlite/values", get(kv::list_sqlite_values))
    .route("/metrics", get(cluster::prometheus_metrics))
    .route(
      "/config",
      get(cluster::get_runtime_config).post(cluster::update_runtime_config),
    )
    .route("/groups", get(cluster::list_groups))
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

#[derive(Deserialize)]
pub(crate) struct ClusterQuery {
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

fn openraft_group_ids(registry: &crate::GroupRegistry) -> Vec<String> {
  registry
    .all()
    .map(|groups| groups.keys().cloned().collect())
    .unwrap_or_default()
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

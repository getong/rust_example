use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use axum::{
  Json, Router,
  extract::State,
  routing::{get, post},
};
use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
  network::{swarm::KvClient, transport::Libp2pNetworkFactory},
  proto::raft_kv::{
    DeleteValueRequest, RaftKvRequest, RaftKvResponse, SetValueRequest,
    UpdateValueRequest as ProtoUpdateValueRequest, raft_kv_request::Op as KvRequestOp,
    raft_kv_response::Op as KvResponseOp,
  },
  typ::{NodeId, Raft},
};

#[derive(Clone)]
pub struct AppState {
  pub node_id: NodeId,
  pub node_name: String,
  pub peer_id: String,
  pub listen: String,
  pub network: Libp2pNetworkFactory,
  pub raft: Raft,
  pub kv_client: KvClient,
}

pub async fn serve(addr: SocketAddr, state: AppState) -> anyhow::Result<()> {
  let app = Router::new()
    .route("/cluster", get(cluster_info))
    .route("/write", post(set_value))
    .route("/update", post(update_value))
    .route("/delete", post(delete_value))
    .with_state(Arc::new(state));

  let listener = tokio::net::TcpListener::bind(addr)
    .await
    .context("bind http")?;
  axum::serve(listener, app).await.context("serve http")?;
  Ok(())
}

#[derive(Serialize)]
struct ClusterInfoResponse {
  node_id: NodeId,
  node_name: String,
  peer_id: String,
  listen: String,
  known_nodes: Vec<KnownNodeResponse>,
  raft_metrics: serde_json::Value,
}

#[derive(Serialize)]
struct KnownNodeResponse {
  node_id: NodeId,
  peer_id: String,
  addr: String,
}

#[derive(Deserialize)]
struct WriteValueRequest {
  key: String,
  #[serde(deserialize_with = "string_or_number")]
  value: String,
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
  target_node_id: Option<NodeId>,
}

#[derive(Serialize)]
struct DeleteValueResponseBody {
  target_node_id: Option<NodeId>,
  ok: bool,
  error: Option<String>,
}

async fn cluster_info(State(state): State<Arc<AppState>>) -> Json<ClusterInfoResponse> {
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

  nodes.sort_by_key(|node| node.node_id);

  let metrics = state.raft.metrics().borrow().clone();
  let raft_metrics = serde_json::to_value(metrics)
    .unwrap_or_else(|err| serde_json::Value::String(format!("metrics serialize error: {err}")));

  Json(ClusterInfoResponse {
    node_id: state.node_id,
    node_name: state.node_name.clone(),
    peer_id: state.peer_id.clone(),
    listen: state.listen.clone(),
    known_nodes: nodes,
    raft_metrics,
  })
}

async fn set_value(
  State(state): State<Arc<AppState>>,
  Json(req): Json<WriteValueRequest>,
) -> Json<WriteValueResponse> {
  let request = RaftKvRequest {
    op: Some(KvRequestOp::Set(SetValueRequest {
      key: req.key,
      value: req.value,
    })),
  };
  let (target_node_id, response) =
    match send_kv_request(state.as_ref(), req.target_node_id, request).await {
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

async fn update_value(
  State(state): State<Arc<AppState>>,
  Json(req): Json<UpdateValueRequest>,
) -> Json<UpdateValueResponse> {
  let request = RaftKvRequest {
    op: Some(KvRequestOp::Update(ProtoUpdateValueRequest {
      key: req.key,
      value: req.value,
    })),
  };
  let (target_node_id, response) =
    match send_kv_request(state.as_ref(), req.target_node_id, request).await {
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
  let request = RaftKvRequest {
    op: Some(KvRequestOp::Delete(DeleteValueRequest { key: req.key })),
  };
  let (target_node_id, response) =
    match send_kv_request(state.as_ref(), req.target_node_id, request).await {
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

async fn send_kv_request(
  state: &AppState,
  target_node_id: Option<NodeId>,
  request: RaftKvRequest,
) -> Result<(NodeId, RaftKvResponse), String> {
  let target = resolve_kv_target(state, target_node_id).await?;
  state.kv_client.dial(target.addr.clone()).await;
  let resp = state
    .kv_client
    .request(target.peer, request)
    .await
    .map_err(|err| format!("libp2p error: {err}"))?;
  Ok((target.node_id, resp))
}

struct KvTarget {
  node_id: NodeId,
  peer: PeerId,
  addr: Multiaddr,
}

async fn resolve_kv_target(
  state: &AppState,
  target_node_id: Option<NodeId>,
) -> Result<KvTarget, String> {
  let nodes = state.network.known_nodes().await;
  if nodes.is_empty() {
    return Err("no known nodes".to_string());
  }

  let metrics = state.raft.metrics().borrow().clone();
  let mut candidate = target_node_id.or_else(|| {
    if metrics.state.is_leader() {
      Some(state.node_id)
    } else {
      metrics.current_leader
    }
  });

  if candidate.is_none() || candidate == Some(state.node_id) {
    if let Some((id, _, _)) = nodes.iter().find(|(id, _, _)| *id != state.node_id) {
      candidate = Some(*id);
    }
  }

  let candidate = candidate.or_else(|| nodes.first().map(|(id, _, _)| *id));

  let Some(node_id) = candidate else {
    return Err("no leader available".to_string());
  };

  nodes
    .into_iter()
    .find(|(id, _, _)| *id == node_id)
    .map(|(id, peer, addr)| KvTarget {
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
  let value = serde_json::Value::deserialize(deserializer)?;
  match value {
    serde_json::Value::String(s) => Ok(s),
    serde_json::Value::Number(n) => Ok(n.to_string()),
    serde_json::Value::Bool(b) => Ok(b.to_string()),
    _ => Err(serde::de::Error::custom("value must be string or number")),
  }
}

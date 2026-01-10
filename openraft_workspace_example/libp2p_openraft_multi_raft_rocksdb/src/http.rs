use std::{
  net::SocketAddr,
  sync::Arc,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use axum::{
  Json, Router,
  extract::{Query, State},
  routing::{get, post},
};
use libp2p::{Multiaddr, PeerId};
use openraft::async_runtime::WatchReceiver;
use prost::Message;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
  GroupHandleMap, GroupId, NodeId,
  network::{
    swarm::{GOSSIP_TOPIC, KvClient},
    transport::Libp2pNetworkFactory,
  },
  proto::raft_kv::{
    ChatMessage, DeleteValueRequest, RaftKvRequest, RaftKvResponse, SetValueRequest,
    UpdateValueRequest as ProtoUpdateValueRequest, raft_kv_request::Op as KvRequestOp,
    raft_kv_response::Op as KvResponseOp,
  },
  signal::ShutdownRx,
  store::ensure_linearizable_read,
};

#[derive(Clone)]
pub struct AppState {
  pub node_id: NodeId,
  pub node_name: String,
  pub peer_id: String,
  pub listen: String,
  pub network: Libp2pNetworkFactory,
  pub kv_client: KvClient,
  pub groups: GroupHandleMap,
  pub default_group: GroupId,
}

pub async fn serve(
  addr: SocketAddr,
  state: AppState,
  mut shutdown_rx: ShutdownRx,
) -> anyhow::Result<()> {
  let app = Router::new()
    .route("/cluster", get(cluster_info))
    .route("/chat", post(send_chat))
    .route("/write", post(set_value))
    .route("/update", post(update_value))
    .route("/delete", post(delete_value))
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
  raft_metrics: serde_json::Value,
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
struct ClusterQuery {
  #[serde(alias = "group")]
  group_id: Option<String>,
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

  nodes.sort_by_key(|node| node.node_id);

  let group_id = query
    .group_id
    .unwrap_or_else(|| state.default_group.clone());

  let groups: Vec<String> = state.groups.keys().cloned().collect();

  let Some(group) = state.groups.get(&group_id) else {
    return Json(ClusterInfoResponse {
      node_id: state.node_id,
      node_name: state.node_name.clone(),
      peer_id: state.peer_id.clone(),
      listen: state.listen.clone(),
      group_id,
      groups,
      known_nodes: nodes,
      raft_metrics: serde_json::Value::String("unknown group".to_string()),
      kv_data: Vec::new(),
      error: Some("unknown group_id".to_string()),
    });
  };

  let metrics = group.raft.metrics().borrow_watched().clone();
  let raft_metrics = serde_json::to_value(metrics)
    .unwrap_or_else(|err| serde_json::Value::String(format!("metrics serialize error: {err}")));

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
    let kvs = group.kv_data.read().await;
    for (key, value) in kvs.iter() {
      kv_data.push(KvPairResponse {
        key: key.clone(),
        value: value.clone(),
      });
    }
  }
  kv_data.sort_by(|a, b| a.key.cmp(&b.key));

  Json(ClusterInfoResponse {
    node_id: state.node_id,
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

async fn send_kv_request(
  state: &AppState,
  group_id: &str,
  target_node_id: Option<NodeId>,
  request: RaftKvRequest,
) -> Result<(NodeId, RaftKvResponse), String> {
  let target = resolve_kv_target(state, group_id, target_node_id).await?;
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

fn resolve_group_id(state: &AppState, group_id: Option<String>) -> Result<GroupId, String> {
  match group_id {
    Some(group_id) => {
      if state.groups.contains_key(&group_id) {
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
  let nodes = state.network.known_nodes().await;
  if nodes.is_empty() {
    return Err("no known nodes".to_string());
  }

  let group = state
    .groups
    .get(group_id)
    .ok_or_else(|| format!("unknown group_id={group_id}"))?;
  let metrics = group.raft.metrics().borrow_watched().clone();
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

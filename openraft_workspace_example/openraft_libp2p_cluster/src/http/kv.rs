use std::sync::Arc;

use axum::extract::State;
use libp2p::{Multiaddr, PeerId};
use openraft::async_runtime::WatchReceiver;
use serde::{
  Deserialize, Deserializer, Serialize,
  de::{self, Visitor},
};

use super::{AppState, Json, resolve_group_id};
use crate::{
  NodeId,
  network::openraft_dispatcher::process_kv_request,
  proto::raft_kv::{
    DeleteValueRequest, RaftKvRequest, RaftKvResponse, SetValueRequest,
    UpdateValueRequest as ProtoUpdateValueRequest, raft_kv_request::Op as KvRequestOp,
    raft_kv_response::Op as KvResponseOp,
  },
  sqlite_cache::{CachedValue, pending_key, record_pending_key},
};

#[derive(Deserialize)]
pub(super) struct WriteValueRequest {
  key: String,
  #[serde(deserialize_with = "string_or_number")]
  value: String,
  group_id: Option<String>,
  target_node_id: Option<NodeId>,
}

#[derive(Serialize)]
pub(super) struct WriteValueResponse {
  target_node_id: Option<NodeId>,
  ok: bool,
  value: Option<String>,
  error: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct UpdateValueRequest {
  key: String,
  #[serde(deserialize_with = "string_or_number")]
  value: String,
  group_id: Option<String>,
  target_node_id: Option<NodeId>,
}

#[derive(Serialize)]
pub(super) struct UpdateValueResponse {
  target_node_id: Option<NodeId>,
  ok: bool,
  value: Option<String>,
  error: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct DeleteValueRequestBody {
  key: String,
  group_id: Option<String>,
  target_node_id: Option<NodeId>,
}

#[derive(Serialize)]
pub(super) struct DeleteValueResponseBody {
  target_node_id: Option<NodeId>,
  ok: bool,
  error: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct CacheWriteRequest {
  key: String,
  #[serde(deserialize_with = "string_or_number")]
  value: String,
  group_id: Option<String>,
  target_node_id: Option<NodeId>,
}

#[derive(Serialize)]
pub(super) struct CacheWriteResponse {
  target_node_id: Option<NodeId>,
  ok: bool,
  pending_key: Option<String>,
  error: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct CacheReadRequest {
  key: String,
}

#[derive(Serialize)]
pub(super) struct CacheReadResponse {
  ok: bool,
  found: bool,
  value: Option<String>,
  error: Option<String>,
}

#[derive(Serialize)]
pub(super) struct SqliteValuesResponse {
  ok: bool,
  values: Vec<CachedValue>,
  error: Option<String>,
}

pub(super) async fn set_value(
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

pub(super) async fn update_value(
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

pub(super) async fn delete_value(
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

pub(super) async fn write_cached_value(
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

pub(super) async fn read_cached_value(
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

pub(super) async fn list_sqlite_values(
  State(state): State<Arc<AppState>>,
) -> Json<SqliteValuesResponse> {
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

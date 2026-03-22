use async_trait::async_trait;
use openraft::async_runtime::WatchReceiver;
use openraft_rocksstore_crud::RocksRequest;
use types_kv::Request as KvWriteRequest;

use crate::{
  GroupHandleMap,
  network::{
    dispatcher::SwarmRequestDispatcher,
    rpc::{RaftRpcOp, RaftRpcRequest, RaftRpcResponse},
    swarm::KvClient,
    transport::parse_p2p_addr,
  },
  proto::raft_kv::{
    ErrorResponse, RaftKvRequest, RaftKvResponse, raft_kv_request::Op as KvRequestOp,
    raft_kv_response::Op as KvResponseOp,
  },
  store::{KvData, ensure_linearizable_read},
  typ::{Raft, Snapshot},
};

#[derive(Clone)]
pub struct OpenRaftDispatcher {
  groups: GroupHandleMap,
  kv_client: KvClient,
}

impl OpenRaftDispatcher {
  pub fn new(groups: GroupHandleMap, kv_client: KvClient) -> Self {
    Self { groups, kv_client }
  }
}

#[async_trait]
impl SwarmRequestDispatcher for OpenRaftDispatcher {
  async fn handle_raft(&self, request: RaftRpcRequest) -> RaftRpcResponse {
    let group_id = request.group_id.clone();
    let Some(group) = self.groups.get(&group_id) else {
      return RaftRpcResponse::Error(format!("unknown group_id={group_id}"));
    };

    handle_inbound_rpc(group.raft.clone(), request.op).await
  }

  async fn handle_kv(&self, request: RaftKvRequest) -> RaftKvResponse {
    let group_id = request.group_id.clone();
    if group_id.is_empty() {
      return kv_error_response("missing group_id");
    }

    let Some(group) = self.groups.get(&group_id) else {
      return kv_error_response(format!("unknown group_id={group_id}"));
    };

    process_kv_request(
      group.raft.clone(),
      group.kv_data.clone(),
      self.kv_client.clone(),
      request,
    )
    .await
  }
}

pub async fn process_kv_request(
  raft: Raft,
  kv_data: KvData,
  kv_client: KvClient,
  request: RaftKvRequest,
) -> RaftKvResponse {
  if request.group_id.is_empty() {
    return kv_error_response("missing group_id");
  }

  let metrics = raft.metrics().borrow_watched().clone();
  if !metrics.state.is_leader() {
    let Some(leader_id) = metrics.current_leader else {
      return kv_error_response("no leader available");
    };
    let Some(node) = metrics.membership_config.membership().get_node(&leader_id) else {
      return kv_error_response("leader node not found in membership");
    };
    let Ok((peer, addr)) = parse_p2p_addr(&node.addr) else {
      return kv_error_response("invalid leader address");
    };
    if let Err(err) = kv_client.connect(peer, addr).await {
      return kv_error_response(format!("connect to leader failed: {err}"));
    }
    return match kv_client.request(peer, request).await {
      Ok(resp) => resp,
      Err(err) => kv_error_response(format!("forward to leader failed: {err}")),
    };
  }

  let Some(op) = request.op else {
    return kv_error_response("missing request op");
  };

  match op {
    KvRequestOp::Get(req) => {
      if let Err(err) = ensure_linearizable_read(&raft).await {
        return kv_error_response(format!("{err:?}"));
      }
      let kvs = kv_data.read().await;
      match kvs.get(&req.key) {
        Some(value) => RaftKvResponse {
          op: Some(KvResponseOp::Get(crate::proto::raft_kv::GetValueResponse {
            found: true,
            value: value.clone(),
          })),
        },
        None => RaftKvResponse {
          op: Some(KvResponseOp::Get(crate::proto::raft_kv::GetValueResponse {
            found: false,
            value: String::new(),
          })),
        },
      }
    }
    KvRequestOp::Set(req) => {
      let key = req.key;
      let value = req.value;
      match raft
        .client_write(KvWriteRequest::Set {
          key: key.clone(),
          value: value.clone(),
        })
        .await
      {
        Ok(resp) => {
          let value = resp.data.value.unwrap_or(value);
          {
            let mut kvs = kv_data.write().await;
            kvs.insert(key, value.clone());
          }
          RaftKvResponse {
            op: Some(KvResponseOp::Set(crate::proto::raft_kv::SetValueResponse {
              ok: true,
              value,
            })),
          }
        }
        Err(err) => kv_error_response(format!("{err:?}")),
      }
    }
    KvRequestOp::Update(req) => {
      let key = req.key;
      let value = req.value;
      if let Err(err) = ensure_linearizable_read(&raft).await {
        return kv_error_response(format!("{err:?}"));
      }
      let exists = {
        let kvs = kv_data.read().await;
        kvs.contains_key(&key)
      };
      if !exists {
        RaftKvResponse {
          op: Some(KvResponseOp::Update(
            crate::proto::raft_kv::UpdateValueResponse {
              ok: false,
              value: String::new(),
            },
          )),
        }
      } else {
        match raft
          .client_write(KvWriteRequest::Set {
            key: key.clone(),
            value: value.clone(),
          })
          .await
        {
          Ok(resp) => {
            let value = resp.data.value.unwrap_or(value);
            {
              let mut kvs = kv_data.write().await;
              kvs.insert(key, value.clone());
            }
            RaftKvResponse {
              op: Some(KvResponseOp::Update(
                crate::proto::raft_kv::UpdateValueResponse { ok: true, value },
              )),
            }
          }
          Err(err) => kv_error_response(format!("{err:?}")),
        }
      }
    }
    KvRequestOp::Delete(req) => {
      if let Err(err) = ensure_linearizable_read(&raft).await {
        return kv_error_response(format!("{err:?}"));
      }
      let exists = {
        let kvs = kv_data.read().await;
        kvs.contains_key(&req.key)
      };
      if !exists {
        RaftKvResponse {
          op: Some(KvResponseOp::Delete(
            crate::proto::raft_kv::DeleteValueResponse { ok: false },
          )),
        }
      } else {
        kv_error_response("delete is not supported by current raft request schema")
      }
    }
  }
}

fn kv_error_response(message: impl Into<String>) -> RaftKvResponse {
  RaftKvResponse {
    op: Some(KvResponseOp::Error(ErrorResponse {
      message: message.into(),
    })),
  }
}

async fn handle_inbound_rpc(raft: Raft, request: RaftRpcOp) -> RaftRpcResponse {
  match request {
    RaftRpcOp::AppendEntries(req) => {
      let res = raft.append_entries(req).await;
      RaftRpcResponse::AppendEntries(res)
    }
    RaftRpcOp::Vote(req) => {
      let res = raft.vote(req).await;
      RaftRpcResponse::Vote(res)
    }
    RaftRpcOp::ClientWrite(req) => {
      let request = match req {
        RocksRequest::Set { key, value } | RocksRequest::Update { key, value } => {
          KvWriteRequest::Set { key, value }
        }
        RocksRequest::Delete { .. } => {
          return RaftRpcResponse::Error(
            "delete is not supported by current raft request schema".to_string(),
          );
        }
      };
      let res = raft.client_write(request).await;
      RaftRpcResponse::ClientWrite(res)
    }
    RaftRpcOp::GetMetrics => {
      let metrics = raft.metrics().borrow_watched().clone();
      RaftRpcResponse::GetMetrics(metrics)
    }
    RaftRpcOp::FullSnapshot { vote, meta, data } => {
      let snapshot = Snapshot {
        meta,
        snapshot: std::io::Cursor::new(data),
      };

      let res = raft
        .install_full_snapshot(vote, snapshot)
        .await
        .map_err(|e| {
          openraft::error::RaftError::<
            openraft_rocksstore_crud::TypeConfig,
            openraft::error::Infallible,
          >::Fatal(e)
        });

      RaftRpcResponse::FullSnapshot(res)
    }
  }
}

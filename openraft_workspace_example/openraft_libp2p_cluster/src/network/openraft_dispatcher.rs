use std::{collections::BTreeSet, time::Duration};

use async_trait::async_trait;
use openraft::{BasicNode, async_runtime::WatchReceiver, log_id::RaftLogId};

use crate::{
  NodeId,
  network::{
    dispatcher::SwarmRequestDispatcher,
    rpc::{JoinClusterRequest, JoinClusterResponse, RaftRpcOp, RaftRpcRequest, RaftRpcResponse},
    swarm::KvClient,
    transport::parse_p2p_addr,
  },
  openraft_group,
  proto::raft_kv::{
    ErrorResponse, RaftKvRequest, RaftKvResponse, raft_kv_request::Op as KvRequestOp,
    raft_kv_response::Op as KvResponseOp,
  },
  rocksstore_crud::{RocksRequest, TypeConfig},
  store::{KvData, ensure_linearizable_read},
  typ::{Raft, Snapshot},
  types_kv::Request as KvWriteRequest,
};

#[derive(Clone)]
pub struct OpenRaftDispatcher {
  kv_client: KvClient,
}

impl OpenRaftDispatcher {
  pub fn new(kv_client: KvClient) -> Self {
    Self { kv_client }
  }
}

#[async_trait]
impl SwarmRequestDispatcher for OpenRaftDispatcher {
  async fn handle_raft(&self, request: RaftRpcRequest) -> RaftRpcResponse {
    let group_id = request.group_id.clone();
    let Some(group) = openraft_group(&group_id) else {
      return RaftRpcResponse::Error(format!("unknown group_id={group_id}"));
    };

    handle_inbound_rpc(group.raft, request.op).await
  }

  async fn handle_kv(&self, request: RaftKvRequest) -> RaftKvResponse {
    let group_id = request.group_id.clone();
    if group_id.is_empty() {
      return kv_error_response("missing group_id");
    }

    let Some(group) = openraft_group(&group_id) else {
      return kv_error_response(format!("unknown group_id={group_id}"));
    };

    process_kv_request(group.raft, group.kv_data, self.kv_client.clone(), request).await
  }

  async fn handle_sqlite_sync(
    &self,
    request: crate::sqlite_sync_rpc::SqliteSyncRpcRequestMessage,
  ) -> crate::sqlite_sync_rpc::SqliteSyncRpcResponseMessage {
    crate::sqlite_cache::process_sqlite_sync_rpc_request(request).await
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
      match kv_data.get(&req.key).await {
        Ok(Some(value)) => RaftKvResponse {
          op: Some(KvResponseOp::Get(crate::proto::raft_kv::GetValueResponse {
            found: true,
            value,
          })),
        },
        Ok(None) => RaftKvResponse {
          op: Some(KvResponseOp::Get(crate::proto::raft_kv::GetValueResponse {
            found: false,
            value: String::new(),
          })),
        },
        Err(err) => kv_error_response(format!("read rocksdb kv failed: {err}")),
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
      let exists = match kv_data.contains_key(&key).await {
        Ok(exists) => exists,
        Err(err) => return kv_error_response(format!("read rocksdb kv failed: {err}")),
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
      let key = req.key;
      if let Err(err) = ensure_linearizable_read(&raft).await {
        return kv_error_response(format!("{err:?}"));
      }
      let exists = match kv_data.contains_key(&key).await {
        Ok(exists) => exists,
        Err(err) => return kv_error_response(format!("read rocksdb kv failed: {err}")),
      };
      if !exists {
        RaftKvResponse {
          op: Some(KvResponseOp::Delete(
            crate::proto::raft_kv::DeleteValueResponse { ok: false },
          )),
        }
      } else {
        match raft.client_write(KvWriteRequest::Delete { key }).await {
          Ok(_) => RaftKvResponse {
            op: Some(KvResponseOp::Delete(
              crate::proto::raft_kv::DeleteValueResponse { ok: true },
            )),
          },
          Err(err) => kv_error_response(format!("{err:?}")),
        }
      }
    }
    KvRequestOp::ListPrefix(req) => {
      if let Err(err) = ensure_linearizable_read(&raft).await {
        return kv_error_response(format!("{err:?}"));
      }

      let entries = match kv_data.entries().await {
        Ok(entries) => entries,
        Err(err) => return kv_error_response(format!("read rocksdb kv failed: {err}")),
      };
      let entries = entries
        .into_iter()
        .filter(|(key, _)| key.starts_with(&req.prefix))
        .map(|(key, value)| crate::proto::raft_kv::KeyValue { key, value })
        .collect();
      RaftKvResponse {
        op: Some(KvResponseOp::ListPrefix(
          crate::proto::raft_kv::ListPrefixResponse { entries },
        )),
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
        RocksRequest::Delete { key } => KvWriteRequest::Delete { key },
      };
      let res = raft.client_write(request).await;
      RaftRpcResponse::ClientWrite(res)
    }
    RaftRpcOp::GetMetrics => {
      let metrics = raft.metrics().borrow_watched().clone();
      RaftRpcResponse::GetMetrics(metrics)
    }
    RaftRpcOp::JoinCluster(req) => {
      let res = handle_join_cluster(raft, req).await;
      RaftRpcResponse::JoinCluster(res)
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
          openraft::error::RaftError::<TypeConfig, openraft::error::Infallible>::Fatal(e)
        });

      RaftRpcResponse::FullSnapshot(res)
    }
  }
}

async fn handle_join_cluster(raft: Raft, req: JoinClusterRequest) -> JoinClusterResponse {
  let metrics = raft.metrics().borrow_watched().clone();
  let membership = metrics.membership_config.membership();
  let before_voters = membership.voter_ids().collect::<BTreeSet<_>>();
  let leader_addr = metrics
    .current_leader
    .as_ref()
    .and_then(|leader_id| membership.get_node(leader_id))
    .map(|node| node.addr.clone());

  if !metrics.state.is_leader() {
    return JoinClusterResponse {
      ok: false,
      joined: false,
      already_member: false,
      voter_count: before_voters.len(),
      max_voters: req.max_voters,
      leader_id: metrics.current_leader,
      leader_addr,
      error: Some("membership changes must be submitted to the leader node".to_string()),
    };
  }

  if before_voters.contains(&req.node_id) {
    return JoinClusterResponse {
      ok: true,
      joined: false,
      already_member: true,
      voter_count: before_voters.len(),
      max_voters: req.max_voters,
      leader_id: metrics.current_leader,
      leader_addr,
      error: None,
    };
  }

  if before_voters.len() >= req.max_voters {
    return JoinClusterResponse {
      ok: true,
      joined: false,
      already_member: false,
      voter_count: before_voters.len(),
      max_voters: req.max_voters,
      leader_id: metrics.current_leader,
      leader_addr,
      error: Some("openraft control membership is full".to_string()),
    };
  }

  let node = BasicNode { addr: req.addr };
  let learner_log_index = match raft.add_learner(req.node_id.clone(), node, false).await {
    Ok(response) => response.log_id.index(),
    Err(err) => {
      return JoinClusterResponse {
        ok: false,
        joined: false,
        already_member: false,
        voter_count: before_voters.len(),
        max_voters: req.max_voters,
        leader_id: metrics.current_leader,
        leader_addr,
        error: Some(format!("add_learner failed: {err:?}")),
      };
    }
  };

  if let Err(err) = wait_for_join_member_rpc(
    raft.clone(),
    &req.node_id,
    learner_log_index,
    Duration::from_millis(req.catch_up_timeout_ms),
  )
  .await
  {
    let metrics = raft.metrics().borrow_watched().clone();
    let voter_count = metrics.membership_config.membership().voter_ids().count();
    return JoinClusterResponse {
      ok: false,
      joined: false,
      already_member: false,
      voter_count,
      max_voters: req.max_voters,
      leader_id: metrics.current_leader,
      leader_addr,
      error: Some(err),
    };
  }

  let voters = raft
    .metrics()
    .borrow_watched()
    .membership_config
    .membership()
    .voter_ids()
    .chain(std::iter::once(req.node_id.clone()))
    .collect::<BTreeSet<_>>();

  match raft.change_membership(voters.clone(), false).await {
    Ok(response) => {
      tracing::info!(
        node_id = %req.node_id,
        response = ?response,
        "joined openraft control membership"
      );
      JoinClusterResponse {
        ok: true,
        joined: true,
        already_member: false,
        voter_count: voters.len(),
        max_voters: req.max_voters,
        leader_id: metrics.current_leader,
        leader_addr,
        error: None,
      }
    }
    Err(err) => JoinClusterResponse {
      ok: false,
      joined: false,
      already_member: false,
      voter_count: voters.len(),
      max_voters: req.max_voters,
      leader_id: metrics.current_leader,
      leader_addr,
      error: Some(format!("change_membership failed: {err:?}")),
    },
  }
}

async fn wait_for_join_member_rpc(
  raft: Raft,
  target_node_id: &NodeId,
  min_matched_index: u64,
  timeout: Duration,
) -> Result<(), String> {
  let deadline = tokio::time::Instant::now() + timeout;
  loop {
    let metrics = raft.metrics().borrow_watched().clone();
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

    tokio::time::sleep(Duration::from_millis(500)).await;
  }
}

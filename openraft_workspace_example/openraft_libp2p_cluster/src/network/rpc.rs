use serde::{Deserialize, Serialize};

use crate::{
  GroupId, NodeId,
  rocksstore_crud::RocksRequest,
  typ::{
    AppendEntriesRequest, AppendEntriesResponse, ClientWriteError, ClientWriteResponse, RaftError,
    RaftMetrics, SnapshotMeta, SnapshotResponse, Vote, VoteRequest, VoteResponse,
  },
};

#[derive(Debug, Serialize, Deserialize)]
pub struct RaftRpcRequest {
  pub group_id: GroupId,
  pub op: RaftRpcOp,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RaftRpcOp {
  AppendEntries(AppendEntriesRequest),
  Vote(VoteRequest),
  ClientWrite(RocksRequest),
  GetMetrics,
  JoinCluster(JoinClusterRequest),
  FullSnapshot {
    vote: Vote,
    meta: SnapshotMeta,
    data: Vec<u8>,
  },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RaftRpcResponse {
  AppendEntries(Result<AppendEntriesResponse, RaftError>),
  Vote(Result<VoteResponse, RaftError>),
  ClientWrite(Result<ClientWriteResponse, RaftError<ClientWriteError>>),
  GetMetrics(RaftMetrics),
  JoinCluster(JoinClusterResponse),
  FullSnapshot(Result<SnapshotResponse, RaftError>),
  Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinClusterRequest {
  pub node_id: NodeId,
  pub addr: String,
  pub max_voters: usize,
  pub catch_up_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinClusterResponse {
  pub ok: bool,
  pub joined: bool,
  pub already_member: bool,
  pub voter_count: usize,
  pub max_voters: usize,
  pub leader_id: Option<NodeId>,
  pub leader_addr: Option<String>,
  pub error: Option<String>,
}

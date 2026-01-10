use openraft_rocksstore_crud::RocksRequest;
use serde::{Deserialize, Serialize};

use crate::{
  GroupId,
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
  FullSnapshot(Result<SnapshotResponse, RaftError>),
  Error(String),
}

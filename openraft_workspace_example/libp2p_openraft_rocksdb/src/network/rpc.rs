use serde::{Deserialize, Serialize};

use crate::typ::{
  AppendEntriesRequest, AppendEntriesResponse, RaftError, SnapshotMeta, SnapshotResponse, Vote,
  VoteRequest, VoteResponse, RaftMetrics,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftRpcRequest {
  AppendEntries(AppendEntriesRequest),
  Vote(VoteRequest),
  GetMetrics,
  FullSnapshot {
    vote: Vote,
    meta: SnapshotMeta,
    data: Vec<u8>,
  },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftRpcResponse {
  AppendEntries(Result<AppendEntriesResponse, RaftError>),
  Vote(Result<VoteResponse, RaftError>),
  GetMetrics(RaftMetrics),
  FullSnapshot(Result<SnapshotResponse, RaftError>),
  Error(String),
}

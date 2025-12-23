use openraft_rocksstore_crud::RocksRequest;
use serde::{Deserialize, Serialize};

use crate::typ::{
  AppendEntriesRequest, AppendEntriesResponse, ClientWriteError, ClientWriteResponse, RaftError,
  RaftMetrics, SnapshotMeta, SnapshotResponse, Vote, VoteRequest, VoteResponse,
};

#[derive(Debug, Serialize, Deserialize)]
pub enum RaftRpcRequest {
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

use serde::{Deserialize, Serialize};

use crate::{
  GroupId, NodeId,
  proto::raft_kv::{RaftKvRequest, RaftKvResponse},
  rocksstore_crud::RocksRequest,
  sqlite_sync_rpc::{SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage},
  tasks::rpc::{TaskRpcRequestMessage, TaskRpcResponseMessage},
  typ::{
    AppendEntriesRequest, AppendEntriesResponse, ClientWriteError, ClientWriteResponse, RaftError,
    RaftMetrics, SnapshotMeta, SnapshotResponse, Vote, VoteRequest, VoteResponse,
  },
  wasm_sync::{WasmSyncRequest, WasmSyncResponse},
};

/// One request enum for the single `/openraft/rpc/2` protocol. All four
/// former request-response protocols (raft / kv / sqlite-sync / task) travel
/// over one libp2p behaviour, so connections negotiate one protocol and the
/// swarm loop keeps one pending table instead of four.
#[derive(Debug)]
pub enum UnifiedRpcRequest {
  Raft(RaftRpcRequest),
  Kv(RaftKvRequest),
  SqliteSync(SqliteSyncRpcRequestMessage),
  Task(TaskRpcRequestMessage),
  WasmSync(WasmSyncRequest),
}

#[derive(Debug)]
pub enum UnifiedRpcResponse {
  Raft(RaftRpcResponse),
  Kv(RaftKvResponse),
  SqliteSync(SqliteSyncRpcResponseMessage),
  Task(TaskRpcResponseMessage),
  WasmSync(WasmSyncResponse),
}

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
  AddLearner(AddLearnerRequest),
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
  AddLearner(AddLearnerRpcResponse),
  FullSnapshot(Result<SnapshotResponse, RaftError>),
  Error(String),
}

/// Add `node_id` as a learner (non-voter) to one raft group. Unlike
/// [`JoinClusterRequest`] it never promotes the node to voter, so it is not
/// subject to the `max_voters` cap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddLearnerRequest {
  pub node_id: NodeId,
  pub addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddLearnerRpcResponse {
  pub ok: bool,
  pub leader_id: Option<NodeId>,
  pub leader_addr: Option<String>,
  pub error: Option<String>,
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

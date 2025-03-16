use openraft::{
  error::InstallSnapshotError,
  raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
  },
};
use serde::{Deserialize, Serialize};

use crate::openraft::TypeConfig;

/// The request type a raft node may send to another
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RaftRequest {
  /// A request to append entries
  AppendEntries(AppendEntriesRequest<TypeConfig>),
  /// A request to install a snapshot
  InstallSnapshot(InstallSnapshotRequest<TypeConfig>),
  /// A request to vote
  Vote(VoteRequest<TypeConfig>),
}

/// The response type a raft node may send to another
#[derive(Debug, Serialize, Deserialize)]
pub enum RaftResponse {
  /// A response to an append entries request
  AppendEntries(AppendEntriesResponse<TypeConfig>),
  /// A response to an install snapshot request
  InstallSnapshot(InstallSnapshotResponse<TypeConfig>),
  /// A response to a vote request
  Vote(VoteResponse<TypeConfig>),
}

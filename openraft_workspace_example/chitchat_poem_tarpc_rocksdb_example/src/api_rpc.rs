use std::io;

use openraft::raft::{
  AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
  VoteRequest, VoteResponse,
};
use serde::{Deserialize, Serialize};
use tarpc::context;
use thiserror::Error;

use crate::{TypeConfig, common::Api};

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
pub enum ServiceError {
  #[error("The git clone operation failed: {}", .0)]
  CloneFailed(String),
  #[error("IO error encountered: {}", .0)]
  IoError(String),
}

impl From<io::Error> for ServiceError {
  fn from(e: io::Error) -> Self {
    Self::IoError(format!("{e}"))
  }
}

#[tarpc::service]
pub trait World {
  async fn vote(vote: VoteRequest<TypeConfig>) -> Result<VoteResponse<TypeConfig>, ServiceError>;
  async fn append(
    req: AppendEntriesRequest<TypeConfig>,
  ) -> Result<AppendEntriesResponse<TypeConfig>, ServiceError>;
  async fn snapshot(
    req: InstallSnapshotRequest<TypeConfig>,
  ) -> Result<InstallSnapshotResponse<TypeConfig>, ServiceError>;
}

impl World for Api {
  async fn vote(
    self,
    _context_info: context::Context,
    vote: VoteRequest<TypeConfig>,
  ) -> Result<VoteResponse<TypeConfig>, ServiceError> {
    self
      .raft
      .vote(vote)
      .await
      .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()).into())
  }

  async fn append(
    self,
    _context_info: context::Context,
    req: AppendEntriesRequest<TypeConfig>,
  ) -> Result<AppendEntriesResponse<TypeConfig>, ServiceError> {
    self
      .raft
      .append_entries(req)
      .await
      .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()).into())
  }
  async fn snapshot(
    self,
    _context_info: context::Context,
    req: InstallSnapshotRequest<TypeConfig>,
  ) -> Result<InstallSnapshotResponse<TypeConfig>, ServiceError> {
    self
      .raft
      .install_snapshot(req)
      .await
      .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()).into())
  }
}

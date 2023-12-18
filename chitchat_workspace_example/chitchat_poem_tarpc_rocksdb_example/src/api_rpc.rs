use crate::common::Api;
use crate::TypeConfig;
use openraft::raft::{
  AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
  VoteRequest, VoteResponse,
};
use tarpc::context;

use serde::{Deserialize, Serialize};
use std::io;
use thiserror::Error;

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
  /// Returns a greeting for name.
  // async fn hello(name: String) -> String;
  async fn vote(vote: VoteRequest<u64>) -> Result<VoteResponse<u64>, ServiceError>;
  async fn append(
    req: AppendEntriesRequest<TypeConfig>,
  ) -> Result<AppendEntriesResponse<u64>, ServiceError>;
  async fn snapshot(
    req: InstallSnapshotRequest<TypeConfig>,
  ) -> Result<InstallSnapshotResponse<u64>, ServiceError>;
}

#[tarpc::server]
impl World for Api {
  // async fn hello(self, _context_info: context::Context, name: String) -> String {
  //   let mut num = self.num.lock().await;
  //   *num += 1;
  //   format!(
  //     "Hello, {name}! You are connected from {}, access num is {}",
  //     name, num
  //   )
  // }

  async fn vote(
    self,
    _context_info: context::Context,
    vote: VoteRequest<u64>,
  ) -> Result<VoteResponse<u64>, ServiceError> {
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
  ) -> Result<AppendEntriesResponse<u64>, ServiceError> {
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
  ) -> Result<InstallSnapshotResponse<u64>, ServiceError> {
    self
      .raft
      .install_snapshot(req)
      .await
      .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()).into())
  }
}

// pub async fn send_hello_msg() -> Result<String, std::io::Error> {
//   let mut transport = tarpc::serde_transport::tcp::connect("127.0.0.1:3000", Json::default);
//   transport.config_mut().max_frame_length(usize::MAX);
//   let client = WorldClient::new(client::Config::default(), transport.await?).spawn();
//   match client
//     .hello(context::current(), format!("{}1", "hello"))
//     .await
//   {
//     Ok(result) => Ok(result),
//     Err(e) => {
//       // Manually handle the conversion from RpcError to std::io::Error
//       let io_error = std::io::Error::new(std::io::ErrorKind::Other, e.to_string());
//       Err(io_error)
//     }
//   }
// }

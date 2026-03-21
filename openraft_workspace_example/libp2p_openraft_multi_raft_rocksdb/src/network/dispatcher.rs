use async_trait::async_trait;

use crate::{
  network::rpc::{RaftRpcRequest, RaftRpcResponse},
  proto::raft_kv::{RaftKvRequest, RaftKvResponse},
};

#[async_trait]
pub trait SwarmRequestDispatcher: Send + Sync + 'static {
  async fn handle_raft(&self, request: RaftRpcRequest) -> RaftRpcResponse;

  async fn handle_kv(&self, request: RaftKvRequest) -> RaftKvResponse;
}

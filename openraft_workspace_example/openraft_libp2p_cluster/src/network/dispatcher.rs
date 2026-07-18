use async_trait::async_trait;

use crate::{
  network::rpc::{RaftRpcRequest, RaftRpcResponse},
  proto::raft_kv::{RaftKvRequest, RaftKvResponse},
  sqlite_sync_rpc::{SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage},
  tasks::rpc::{TaskRpcRequestMessage, TaskRpcResponseMessage},
  wasm_sync::{WasmSyncRequest, WasmSyncResponse},
};

#[async_trait]
pub trait SwarmRequestDispatcher: Send + Sync + 'static {
  async fn handle_raft(&self, request: RaftRpcRequest) -> RaftRpcResponse;

  async fn handle_kv(&self, request: RaftKvRequest) -> RaftKvResponse;

  async fn handle_sqlite_sync(
    &self,
    request: SqliteSyncRpcRequestMessage,
  ) -> SqliteSyncRpcResponseMessage;

  async fn handle_task_rpc(&self, request: TaskRpcRequestMessage) -> TaskRpcResponseMessage;

  async fn handle_wasm_sync(&self, request: WasmSyncRequest) -> WasmSyncResponse;
}

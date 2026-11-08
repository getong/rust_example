use std::future::Future;

use openraft::{
  BasicNode, OptionalSend, RaftNetworkFactory,
  error::ReplicationClosed,
  network::{RPCOption, v2::RaftNetworkV2},
  raft::AppendResult,
};

use crate::{NodeId, TypeConfig, router::Router, typ::*};

pub struct Connection {
  router: Router,
  target: NodeId,
}

impl RaftNetworkFactory<TypeConfig> for Router {
  type Network = Connection;

  async fn new_client(&mut self, target: NodeId, _node: &BasicNode) -> Self::Network {
    Connection {
      router: self.clone(),
      target,
    }
  }
}

impl RaftNetworkV2<TypeConfig> for Connection {
  type SnapshotData = crate::SnapshotData;

  async fn append_entries(
    &mut self,
    req: AppendEntriesRequest,
    _option: RPCOption,
  ) -> Result<AppendResult<TypeConfig>, RPCError> {
    let last_log_id = req.last_log_id();
    let prev_log_id = req.prev_log_id;
    let resp: AppendEntriesResponse = self.router.send(self.target, "/raft/append", req).await?;
    Ok(resp.into_append_result(prev_log_id, last_log_id))
  }

  /// A real application should replace this method with customized implementation.
  async fn full_snapshot(
    &mut self,
    vote: Vote,
    snapshot: Snapshot,
    _cancel: impl Future<Output = ReplicationClosed> + OptionalSend + 'static,
    _option: RPCOption,
  ) -> Result<SnapshotResponse, StreamingError> {
    // Extract inner Vec<u8> from Cursor for serialization
    let data: Vec<u8> = snapshot.snapshot.into_inner();
    let resp = self
      .router
      .send(self.target, "/raft/snapshot", (vote, snapshot.meta, data))
      .await?;
    Ok(resp)
  }

  async fn vote(&mut self, req: VoteRequest, _option: RPCOption) -> Result<VoteResponse, RPCError> {
    let resp = self.router.send(self.target, "/raft/vote", req).await?;
    Ok(resp)
  }
}

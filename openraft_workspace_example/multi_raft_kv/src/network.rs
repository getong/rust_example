use std::future::Future;

use openraft::{
  OptionalSend,
  error::{RPCError, ReplicationClosed, StreamingError},
  network::{Backoff, RPCOption, RaftNetworkFactory},
  raft::{
    AppendEntriesRequest, AppendEntriesResponse, SnapshotResponse, TransferLeaderRequest,
    VoteRequest, VoteResponse,
  },
  storage::Snapshot,
};
use openraft_multi::{GroupNetworkAdapter, GroupNetworkFactory, GroupRouter};

use crate::{GroupId, NodeId, TypeConfig, router::Router, typ};

impl GroupRouter<TypeConfig, GroupId> for Router {
  async fn append_entries(
    &self,
    target: NodeId,
    group_id: GroupId,
    rpc: AppendEntriesRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<AppendEntriesResponse<TypeConfig>, RPCError<TypeConfig>> {
    self
      .send(target, &group_id, "/raft/append", rpc)
      .await
      .map_err(RPCError::Unreachable)
  }

  async fn vote(
    &self,
    target: NodeId,
    group_id: GroupId,
    rpc: VoteRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<VoteResponse<TypeConfig>, RPCError<TypeConfig>> {
    self
      .send(target, &group_id, "/raft/vote", rpc)
      .await
      .map_err(RPCError::Unreachable)
  }

  async fn full_snapshot(
    &self,
    target: NodeId,
    group_id: GroupId,
    vote: typ::Vote,
    snapshot: Snapshot<TypeConfig>,
    _cancel: impl Future<Output = ReplicationClosed> + OptionalSend + 'static,
    _option: RPCOption,
  ) -> Result<SnapshotResponse<TypeConfig>, StreamingError<TypeConfig>> {
    self
      .send(
        target,
        &group_id,
        "/raft/snapshot",
        (vote, snapshot.meta, snapshot.snapshot),
      )
      .await
      .map_err(StreamingError::Unreachable)
  }

  async fn transfer_leader(
    &self,
    target: NodeId,
    group_id: GroupId,
    req: TransferLeaderRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<(), RPCError<TypeConfig>> {
    self
      .send(target, &group_id, "/raft/transfer_leader", req)
      .await
      .map_err(RPCError::Unreachable)
  }

  fn backoff(&self) -> Backoff {
    Backoff::new(std::iter::repeat(std::time::Duration::from_millis(500)))
  }
}

/// Network factory that creates `GroupNetworkAdapter` instances.
pub type NetworkFactory = GroupNetworkFactory<Router, GroupId>;

impl RaftNetworkFactory<TypeConfig> for NetworkFactory {
  /// The network type is `GroupNetworkAdapter` binding (Router, target, group_id).
  type Network = GroupNetworkAdapter<TypeConfig, GroupId, Router>;

  async fn new_client(&mut self, target: NodeId, _node: &openraft::BasicNode) -> Self::Network {
    GroupNetworkAdapter::new(self.factory.clone(), target, self.group_id.clone())
  }
}

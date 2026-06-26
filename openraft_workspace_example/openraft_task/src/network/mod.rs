use std::{collections::BTreeMap, future::Future, sync::Arc};

use openraft::{
  BasicNode, OptionalSend, RaftNetworkFactory,
  error::{NetworkError, RPCError, ReplicationClosed, StreamingError, Unreachable},
  errors::decompose::DecomposeResult,
  network::{RPCOption, v2::RaftNetworkV2},
};
use tokio::sync::RwLock;

use crate::{
  Raft, TypeConfig,
  typ::{
    AppendEntriesRequest, AppendEntriesResponse, Snapshot, SnapshotMeta, SnapshotResponse, Vote,
    VoteRequest, VoteResponse,
  },
};

#[derive(Debug, Clone, Default)]
pub struct Router {
  nodes: Arc<RwLock<BTreeMap<u64, Raft>>>,
}

impl Router {
  pub async fn insert(&self, node_id: u64, raft: Raft) {
    self.nodes.write().await.insert(node_id, raft);
  }

  async fn get(&self, node_id: u64) -> Result<Raft, RPCError<TypeConfig>> {
    self
      .nodes
      .read()
      .await
      .get(&node_id)
      .cloned()
      .ok_or_else(|| RPCError::Network(NetworkError::new(&RouterError(node_id))))
  }
}

#[derive(Debug)]
struct RouterError(u64);

impl std::fmt::Display for RouterError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "raft node {} is not registered", self.0)
  }
}

impl std::error::Error for RouterError {}

pub struct Connection {
  router: Router,
  target: u64,
}

impl RaftNetworkFactory<TypeConfig> for Router {
  type Network = Connection;

  async fn new_client(&mut self, target: u64, _node: &BasicNode) -> Self::Network {
    Connection {
      router: self.clone(),
      target,
    }
  }
}

impl RaftNetworkV2<TypeConfig> for Connection {
  async fn append_entries(
    &mut self,
    req: AppendEntriesRequest,
    _option: RPCOption,
  ) -> Result<AppendEntriesResponse, RPCError<TypeConfig>> {
    self
      .router
      .get(self.target)
      .await?
      .append_entries(req)
      .await
      .map_err(|err| RPCError::RemoteError(openraft::error::RemoteError::new(self.target, err)))
      .decompose_infallible()
  }

  async fn vote(
    &mut self,
    req: VoteRequest,
    _option: RPCOption,
  ) -> Result<VoteResponse, RPCError<TypeConfig>> {
    self
      .router
      .get(self.target)
      .await?
      .vote(req)
      .await
      .map_err(|err| RPCError::RemoteError(openraft::error::RemoteError::new(self.target, err)))
      .decompose_infallible()
  }

  async fn full_snapshot(
    &mut self,
    vote: Vote,
    snapshot: Snapshot,
    _cancel: impl Future<Output = ReplicationClosed> + OptionalSend + 'static,
    _option: RPCOption,
  ) -> Result<SnapshotResponse, StreamingError<TypeConfig>> {
    let target = self.router.get(self.target).await?;
    let snapshot = Snapshot {
      meta: SnapshotMeta {
        last_log_id: snapshot.meta.last_log_id,
        last_membership: snapshot.meta.last_membership,
        snapshot_id: snapshot.meta.snapshot_id,
      },
      snapshot: snapshot.snapshot,
    };
    target
      .install_full_snapshot(vote, snapshot)
      .await
      .map_err(|err| StreamingError::Unreachable(Unreachable::new(&err)))
  }
}

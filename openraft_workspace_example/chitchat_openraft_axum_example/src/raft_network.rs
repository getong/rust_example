use std::future::Future;
use openraft::error::ReplicationClosed;
use openraft::network::v2::RaftNetworkV2;
use openraft::network::RPCOption;
use openraft::BasicNode;
use openraft::OptionalSend;
use openraft::RaftNetworkFactory;

use crate::raft_router::Router;
use crate::raft_types::{NodeId, TypeConfig};

pub type AppendEntriesRequest = openraft::raft::AppendEntriesRequest<TypeConfig>;
pub type AppendEntriesResponse = openraft::raft::AppendEntriesResponse<TypeConfig>;
pub type VoteRequest = openraft::raft::VoteRequest<TypeConfig>;
pub type VoteResponse = openraft::raft::VoteResponse<TypeConfig>;
pub type Snapshot = openraft::storage::Snapshot<TypeConfig>;
pub type SnapshotResponse = openraft::raft::SnapshotResponse<TypeConfig>;
pub type Vote = openraft::Vote<NodeId>;
pub type RPCError = openraft::error::RaftError<TypeConfig, openraft::error::Unreachable>;
pub type StreamingError = openraft::error::RaftError<TypeConfig, openraft::error::StreamingError<TypeConfig, openraft::error::Unreachable>>;

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
    async fn append_entries(
        &mut self,
        req: AppendEntriesRequest,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse, RPCError> {
        let resp = self.router.send(self.target, "/raft/append", req).await?;
        Ok(resp)
    }

    /// A real application should replace this method with customized implementation.
    async fn full_snapshot(
        &mut self,
        vote: Vote,
        snapshot: Snapshot,
        _cancel: impl Future<Output = ReplicationClosed> + OptionalSend + 'static,
        _option: RPCOption,
    ) -> Result<SnapshotResponse, StreamingError> {
        let resp = self.router.send(
            self.target,
            "/raft/snapshot",
            (vote, snapshot.meta, snapshot.snapshot)
        ).await?;
        Ok(resp)
    }

    async fn vote(&mut self, req: VoteRequest, _option: RPCOption) -> Result<VoteResponse, RPCError> {
        let resp = self.router.send(self.target, "/raft/vote", req).await?;
        Ok(resp)
    }
}

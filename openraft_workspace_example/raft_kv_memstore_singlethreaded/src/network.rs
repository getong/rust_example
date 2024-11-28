use openraft::{
  error::{InstallSnapshotError, RemoteError},
  network::RPCOption,
  raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
  },
  BasicNode, RaftNetwork, RaftNetworkFactory,
};

use crate::{router::Router, typ, NodeId, TypeConfig};

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

impl RaftNetwork<TypeConfig> for Connection {
  async fn append_entries(
    &mut self,
    req: AppendEntriesRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<AppendEntriesResponse<TypeConfig>, typ::RPCError> {
    let resp = self
      .router
      .send(self.target, "/raft/append", req)
      .await
      .map_err(|e| RemoteError::new(self.target, e))?;
    Ok(resp)
  }

  async fn install_snapshot(
    &mut self,
    req: InstallSnapshotRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<InstallSnapshotResponse<TypeConfig>, typ::RPCError<InstallSnapshotError>> {
    let resp = self
      .router
      .send(self.target, "/raft/snapshot", req)
      .await
      .map_err(|e| RemoteError::new(self.target, e))?;
    Ok(resp)
  }

  async fn vote(
    &mut self,
    req: VoteRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<VoteResponse<TypeConfig>, typ::RPCError> {
    let resp = self
      .router
      .send(self.target, "/raft/vote", req)
      .await
      .map_err(|e| RemoteError::new(self.target, e))?;
    Ok(resp)
  }
}

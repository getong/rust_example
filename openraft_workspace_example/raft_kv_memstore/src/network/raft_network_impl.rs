use openraft::{
  error::{InstallSnapshotError, NetworkError, RemoteError, Unreachable},
  network::{RPCOption, RaftNetwork, RaftNetworkFactory},
  raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
  },
  BasicNode,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::{typ::*, NodeId, TypeConfig};

pub struct Network {}

impl Network {
  pub async fn send_rpc<Req, Resp, Err>(
    &self,
    target: NodeId,
    target_node: &BasicNode,
    uri: &str,
    req: Req,
  ) -> Result<Resp, openraft::error::RPCError<TypeConfig, Err>>
  where
    Req: Serialize,
    Err: std::error::Error + DeserializeOwned,
    Resp: DeserializeOwned,
  {
    let addr = &target_node.addr;

    let url = format!("http://{}/{}", addr, uri);
    tracing::debug!("send_rpc to url: {}", url);

    let client = reqwest::Client::new();
    tracing::debug!("client is created for: {}", url);

    let resp = client.post(url).json(&req).send().await.map_err(|e| {
      // If the error is a connection error, we return `Unreachable` so that connection isn't
      // retried immediately.
      if e.is_connect() {
        return openraft::error::RPCError::Unreachable(Unreachable::new(&e));
      }
      openraft::error::RPCError::Network(NetworkError::new(&e))
    })?;

    tracing::debug!("client.post() is sent");

    let res: Result<Resp, Err> = resp
      .json()
      .await
      .map_err(|e| openraft::error::RPCError::Network(NetworkError::new(&e)))?;

    res.map_err(|e| openraft::error::RPCError::RemoteError(RemoteError::new(target, e)))
  }
}

// NOTE: This could be implemented also on `Arc<ExampleNetwork>`, but since it's empty, implemented
// directly.
impl RaftNetworkFactory<TypeConfig> for Network {
  type Network = NetworkConnection;

  async fn new_client(&mut self, target: NodeId, node: &BasicNode) -> Self::Network {
    NetworkConnection {
      owner: Network {},
      target,
      target_node: node.clone(),
    }
  }
}

pub struct NetworkConnection {
  owner: Network,
  target: NodeId,
  target_node: BasicNode,
}

impl RaftNetwork<TypeConfig> for NetworkConnection {
  async fn append_entries(
    &mut self,
    req: AppendEntriesRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<AppendEntriesResponse<TypeConfig>, RPCError<RaftError>> {
    self
      .owner
      .send_rpc(self.target, &self.target_node, "raft-append", req)
      .await
  }

  async fn install_snapshot(
    &mut self,
    req: InstallSnapshotRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<InstallSnapshotResponse<TypeConfig>, RPCError<RaftError<InstallSnapshotError>>> {
    self
      .owner
      .send_rpc(self.target, &self.target_node, "raft-snapshot", req)
      .await
  }

  async fn vote(
    &mut self,
    req: VoteRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<VoteResponse<TypeConfig>, RPCError<RaftError>> {
    self
      .owner
      .send_rpc(self.target, &self.target_node, "raft-vote", req)
      .await
  }
}

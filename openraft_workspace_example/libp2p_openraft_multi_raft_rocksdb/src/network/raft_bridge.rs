use std::{error::Error, fmt, future::Future, sync::Arc};

use async_trait::async_trait;
use openraft::{
  BasicNode, RaftNetworkFactory,
  network::{RPCOption, v2::RaftNetworkV2},
};

use crate::{
  GroupId, NodeId, TypeConfig, Unreachable,
  network::rpc::{RaftRpcOp, RaftRpcRequest, RaftRpcResponse},
  typ::{
    AppendEntriesRequest, AppendEntriesResponse, RPCError, Snapshot, SnapshotResponse,
    StreamingError, Vote, VoteRequest, VoteResponse,
  },
};

#[derive(Debug, Clone)]
struct BridgeErr(String);

impl BridgeErr {
  fn new(message: impl Into<String>) -> Self {
    Self(message.into())
  }
}

impl fmt::Display for BridgeErr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl Error for BridgeErr {}

#[async_trait]
pub trait P2PRaftNetwork: Send + Sync + 'static {
  fn target(&self) -> NodeId;

  fn group_id(&self) -> &GroupId;

  async fn send_request(
    &self,
    target: NodeId,
    request: RaftRpcRequest,
  ) -> Result<RaftRpcResponse, Unreachable>;
}

pub struct P2PRaftNetworkWrapper {
  inner: Box<dyn P2PRaftNetwork + Send + Sync>,
}

impl P2PRaftNetworkWrapper {
  pub fn new<N: P2PRaftNetwork>(inner: N) -> Self {
    Self {
      inner: Box::new(inner),
    }
  }

  async fn send_op(&self, op: RaftRpcOp) -> Result<RaftRpcResponse, Unreachable> {
    let target = self.inner.target();
    let request = RaftRpcRequest {
      group_id: self.inner.group_id().clone(),
      op,
    };
    self.inner.send_request(target, request).await
  }
}

#[async_trait]
impl P2PRaftNetwork for P2PRaftNetworkWrapper {
  fn target(&self) -> NodeId {
    self.inner.target()
  }

  fn group_id(&self) -> &GroupId {
    self.inner.group_id()
  }

  async fn send_request(
    &self,
    target: NodeId,
    request: RaftRpcRequest,
  ) -> Result<RaftRpcResponse, Unreachable> {
    self.inner.send_request(target, request).await
  }
}

impl RaftNetworkV2<TypeConfig> for P2PRaftNetworkWrapper {
  async fn append_entries(
    &mut self,
    req: AppendEntriesRequest,
    _option: RPCOption,
  ) -> Result<AppendEntriesResponse, RPCError> {
    let response = self
      .send_op(RaftRpcOp::AppendEntries(req))
      .await
      .map_err(RPCError::Unreachable)?;
    match response {
      RaftRpcResponse::AppendEntries(result) => {
        result.map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))
      }
      other => Err(RPCError::Unreachable(Unreachable::new(&BridgeErr::new(
        format!("unexpected response: {other:?}"),
      )))),
    }
  }

  async fn vote(&mut self, req: VoteRequest, _option: RPCOption) -> Result<VoteResponse, RPCError> {
    let response = self
      .send_op(RaftRpcOp::Vote(req))
      .await
      .map_err(RPCError::Unreachable)?;
    match response {
      RaftRpcResponse::Vote(result) => {
        result.map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))
      }
      other => Err(RPCError::Unreachable(Unreachable::new(&BridgeErr::new(
        format!("unexpected response: {other:?}"),
      )))),
    }
  }

  async fn full_snapshot(
    &mut self,
    vote: Vote,
    snapshot: Snapshot,
    _cancel: impl Future<Output = openraft::error::ReplicationClosed> + openraft::OptionalSend + 'static,
    _option: RPCOption,
  ) -> Result<SnapshotResponse, StreamingError> {
    let data = snapshot.snapshot.into_inner();
    let response = self
      .send_op(RaftRpcOp::FullSnapshot {
        vote,
        meta: snapshot.meta,
        data,
      })
      .await
      .map_err(StreamingError::Unreachable)?;
    match response {
      RaftRpcResponse::FullSnapshot(result) => {
        result.map_err(|err| StreamingError::Unreachable(Unreachable::new(&err)))
      }
      other => Err(StreamingError::Unreachable(Unreachable::new(
        &BridgeErr::new(format!("unexpected response: {other:?}")),
      ))),
    }
  }
}

#[async_trait]
pub trait P2PNetworkFactory: Send + Sync + 'static {
  async fn new_p2p_client(&self, target: NodeId, target_info: BasicNode) -> P2PRaftNetworkWrapper;
}

#[derive(Clone)]
pub struct P2PNetworkFactoryWrapper {
  inner: Arc<dyn P2PNetworkFactory>,
}

impl P2PNetworkFactoryWrapper {
  pub fn new<F: P2PNetworkFactory>(factory: F) -> Self {
    Self {
      inner: Arc::new(factory),
    }
  }
}

#[async_trait]
impl P2PNetworkFactory for P2PNetworkFactoryWrapper {
  async fn new_p2p_client(&self, target: NodeId, target_info: BasicNode) -> P2PRaftNetworkWrapper {
    self.inner.new_p2p_client(target, target_info).await
  }
}

impl RaftNetworkFactory<TypeConfig> for P2PNetworkFactoryWrapper {
  type Network = P2PRaftNetworkWrapper;

  async fn new_client(&mut self, target: NodeId, target_info: &BasicNode) -> Self::Network {
    self.inner.new_p2p_client(target, target_info.clone()).await
  }
}

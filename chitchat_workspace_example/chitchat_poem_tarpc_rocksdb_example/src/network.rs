use std::fmt::Display;

use async_trait::async_trait;
use openraft::error::InstallSnapshotError;
use openraft::error::NetworkError;
use openraft::error::RPCError;
use openraft::error::RaftError;

use crate::api_rpc::ServiceError;
use crate::api_rpc::WorldClient;
use openraft::network::RaftNetwork;
use openraft::network::RaftNetworkFactory;
use openraft::raft::AppendEntriesRequest;
use openraft::raft::AppendEntriesResponse;
use openraft::raft::InstallSnapshotRequest;
use openraft::raft::InstallSnapshotResponse;
use openraft::raft::VoteRequest;
use openraft::raft::VoteResponse;
use openraft::AnyError;
use serde::de::DeserializeOwned;

use tarpc::{client, context, tokio_serde::formats::Json};

use crate::Node;
use crate::NodeId;
use crate::TypeConfig;

pub struct Network {}

// NOTE: This could be implemented also on `Arc<ExampleNetwork>`, but since it's empty, implemented
// directly.
#[async_trait]
impl RaftNetworkFactory<TypeConfig> for Network {
  type Network = NetworkConnection;

  #[tracing::instrument(level = "debug", skip_all)]
  async fn new_client(&mut self, target: NodeId, node: &Node) -> Self::Network {
    let mut transport = tarpc::serde_transport::tcp::connect(&node.rpc_addr, Json::default);
    transport.config_mut().max_frame_length(usize::MAX);
    let client = WorldClient::new(client::Config::default(), transport.await.unwrap()).spawn();
    tracing::debug!("new_client: is_none: {:?}", client);

    NetworkConnection {
      addr: node.rpc_addr.clone(),
      client: Some(client),
      target,
    }
  }
}

pub struct NetworkConnection {
  addr: String,
  client: Option<WorldClient>,
  target: NodeId,
}
impl NetworkConnection {
  async fn c<E: std::error::Error + DeserializeOwned>(
    &mut self,
  ) -> Result<&WorldClient, RPCError<NodeId, Node, E>> {
    if self.client.is_none() {
      let mut transport = tarpc::serde_transport::tcp::connect(&self.addr, Json::default);
      transport.config_mut().max_frame_length(usize::MAX);
      self.client =
        Some(WorldClient::new(client::Config::default(), transport.await.unwrap()).spawn());
    }
    self
      .client
      .as_ref()
      .ok_or_else(|| RPCError::Network(NetworkError::from(AnyError::default())))
  }
}

#[derive(Debug)]
struct ErrWrap(Box<dyn std::error::Error>);

impl Display for ErrWrap {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(f)
  }
}

impl std::error::Error for ErrWrap {}

fn to_error<E: std::error::Error + 'static + Clone>(
  _e: ServiceError,
  _target: NodeId,
) -> RPCError<NodeId, Node, E> {
  // match e {
  //   toy_rpc::Error::IoError(e) => RPCError::Network(NetworkError::new(&e)),
  //   toy_rpc::Error::ParseError(e) => RPCError::Network(NetworkError::new(&ErrWrap(e))),
  //   toy_rpc::Error::Internal(e) => {
  //     let any: &dyn Any = &e;
  //     let error: &E = any.downcast_ref().unwrap();
  //     RPCError::RemoteError(RemoteError::new(target, error.clone()))
  //   }
  //   e @ (toy_rpc::Error::InvalidArgument
  //   | toy_rpc::Error::ServiceNotFound
  //   | toy_rpc::Error::MethodNotFound
  //   | toy_rpc::Error::ExecutionError(_)
  //   | toy_rpc::Error::Canceled(_)
  //   | toy_rpc::Error::Timeout(_)
  //   | toy_rpc::Error::MaxRetriesReached(_)) => RPCError::Network(NetworkError::new(&e)),
  // }
  RPCError::Network(NetworkError::from(AnyError::default()))
}

#[async_trait]
impl RaftNetwork<TypeConfig> for NetworkConnection {
  #[tracing::instrument(level = "debug", skip_all, err(Debug))]
  async fn send_append_entries(
    &mut self,
    req: AppendEntriesRequest<TypeConfig>,
  ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId>>> {
    tracing::debug!(req = debug(&req), "send_append_entries");

    let client = self.c().await?;
    tracing::debug!("got connection");

    // let raft = c.raft();
    // tracing::debug!("got raft");

    client
      .append(context::current(), req)
      .await
      .unwrap()
      .map_err(|e| to_error(e, self.target))
  }

  #[tracing::instrument(level = "debug", skip_all, err(Debug))]
  async fn send_install_snapshot(
    &mut self,
    req: InstallSnapshotRequest<TypeConfig>,
  ) -> Result<
    InstallSnapshotResponse<NodeId>,
    RPCError<NodeId, Node, RaftError<NodeId, InstallSnapshotError>>,
  > {
    tracing::debug!(req = debug(&req), "send_install_snapshot");
    self
      .c()
      .await?
      .snapshot(context::current(), req)
      .await
      .unwrap()
      .map_err(|e| to_error(e, self.target))
  }

  #[tracing::instrument(level = "debug", skip_all, err(Debug))]
  async fn send_vote(
    &mut self,
    req: VoteRequest<NodeId>,
  ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId>>> {
    tracing::debug!(req = debug(&req), "send_vote");
    self
      .c()
      .await?
      .vote(context::current(), req)
      .await
      .unwrap()
      .map_err(|e| to_error(e, self.target))
  }
}

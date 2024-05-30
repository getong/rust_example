// use std::fmt::Display;
use openraft::error::InstallSnapshotError;
use openraft::error::NetworkError;
use openraft::error::RPCError;
use openraft::error::RaftError;
use openraft::network::RPCOption;

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
  ) -> Result<&WorldClient, RPCError<TypeConfig, E>> {
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

fn to_error<E: std::error::Error + 'static + Clone>(
  _e: ServiceError,
  _target: NodeId,
) -> RPCError<TypeConfig, E> {
  RPCError::Network(NetworkError::from(AnyError::default()))
}

#[allow(clippy::blocks_in_conditions)]
impl RaftNetwork<TypeConfig> for NetworkConnection {
  #[tracing::instrument(level = "debug", skip_all, err(Debug))]
  async fn append_entries(
    &mut self,
    req: AppendEntriesRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<AppendEntriesResponse<TypeConfig>, RPCError<TypeConfig, RaftError<TypeConfig>>> {
    tracing::debug!(req = debug(&req), "append_entries");

    let client = self.c().await?;
    tracing::debug!("got connection");

    client
      .append(context::current(), req)
      .await
      .unwrap()
      .map_err(|e| to_error(e, self.target))
  }

  #[tracing::instrument(level = "debug", skip_all, err(Debug))]
  async fn install_snapshot(
    &mut self,
    req: InstallSnapshotRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<
    InstallSnapshotResponse<TypeConfig>,
    RPCError<TypeConfig, RaftError<TypeConfig, InstallSnapshotError>>,
  > {
    tracing::debug!(req = debug(&req), "install_snapshot");
    self
      .c()
      .await?
      .snapshot(context::current(), req)
      .await
      .unwrap()
      .map_err(|e| to_error(e, self.target))
  }

  #[tracing::instrument(level = "debug", skip_all, err(Debug))]
  async fn vote(
    &mut self,
    req: VoteRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<VoteResponse<TypeConfig>, RPCError<TypeConfig, RaftError<TypeConfig>>> {
    tracing::debug!(req = debug(&req), "vote");
    self
      .c()
      .await?
      .vote(context::current(), req)
      .await
      .unwrap()
      .map_err(|e| to_error(e, self.target))
  }
}

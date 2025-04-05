use std::io;

use futures::Future;
use openraft::{
  AnyError,
  error::{InstallSnapshotError, NetworkError, RPCError, RaftError},
  network::{RPCOption, RaftNetwork, RaftNetworkFactory},
  raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
  },
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;
use tokio::sync::oneshot::{Receiver as OneshotReceiver, Sender as OneshotSender, channel};

use crate::{
  libp2p::{
    LAZY_EVENT_SENDER,
    behaviour::{RaftRequest, RaftResponse},
  },
  openraft::{Node, NodeId, TypeConfig},
};

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
pub enum ServiceError {
  #[error("The git clone operation failed: {}", .0)]
  CloneFailed(String),
  #[error("IO error encountered: {}", .0)]
  IoError(String),
}

impl From<io::Error> for ServiceError {
  fn from(e: io::Error) -> Self {
    Self::IoError(format!("{e}"))
  }
}

pub struct Network {}

// NOTE: This could be implemented also on `Arc<ExampleNetwork>`, but since it's empty, implemented
// directly.
impl RaftNetworkFactory<TypeConfig> for Network {
  type Network = NetworkConnection;

  #[tracing::instrument(level = "debug", skip_all)]
  async fn new_client(&mut self, target: NodeId, node: &Node) -> Self::Network {
    NetworkConnection {}
  }
}

#[derive(Debug)]
pub struct NetworkConnection {}

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

    let (tx, rx) = channel();
    let sender_lock = LAZY_EVENT_SENDER.lock();
    if let Some(sender) = sender_lock.await.as_ref() {
      if let Err(e) = sender.send((RaftRequest::AppendEntries(req), tx)).await {
        eprintln!("Failed to send event: {:?}", e);
      }
    } else {
      eprintln!("Event sender is not initialized");
    }
    if let RaftResponse::AppendEntries(resp) = rx
      .await
      .map_err(|_e| RPCError::Network(NetworkError::from(AnyError::default())))?
    {
      Ok(resp)
    } else {
      panic!("Expected Vote response, got {:?}", self);
    }
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
    let (tx, rx) = channel();
    let sender_lock = LAZY_EVENT_SENDER.lock();
    if let Some(sender) = sender_lock.await.as_ref() {
      if let Err(e) = sender.send((RaftRequest::InstallSnapshot(req), tx)).await {
        eprintln!("Failed to send event: {:?}", e);
      }
    } else {
      eprintln!("Event sender is not initialized");
    }
    if let RaftResponse::InstallSnapshot(resp) = rx
      .await
      .map_err(|_e| RPCError::Network(NetworkError::from(AnyError::default())))?
    {
      Ok(resp)
    } else {
      panic!("Expected Vote response, got {:?}", self);
    }
  }

  #[tracing::instrument(level = "debug", skip_all, err(Debug))]
  async fn vote(
    &mut self,
    req: VoteRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<VoteResponse<TypeConfig>, RPCError<TypeConfig, RaftError<TypeConfig>>> {
    tracing::debug!(req = debug(&req), "vote");
    let (tx, rx) = channel();
    let sender_lock = LAZY_EVENT_SENDER.lock();
    if let Some(sender) = sender_lock.await.as_ref() {
      if let Err(e) = sender.send((RaftRequest::Vote(req), tx)).await {
        eprintln!("Failed to send event: {:?}", e);
      }
    } else {
      eprintln!("Event sender is not initialized");
    }
    if let RaftResponse::Vote(resp) = rx
      .await
      .map_err(|_e| RPCError::Network(NetworkError::from(AnyError::default())))?
    {
      Ok(resp)
    } else {
      panic!("Expected Vote response, got {:?}", self);
    }
  }
}

use std::{
  collections::HashMap,
  sync::{Arc, Mutex},
};

use openraft::{
  RaftNetwork, RaftNetworkFactory,
  error::{InstallSnapshotError, RPCError},
};
use tokio::sync::{mpsc, oneshot};

use crate::raft_simple_types::*;

pub type ResponseTx = oneshot::Sender<String>;
pub type RequestTx = mpsc::UnboundedSender<(String, String, ResponseTx)>;

/// Router for sending messages between Raft nodes
#[derive(Debug, Clone)]
pub struct Router {
  pub targets: Arc<Mutex<HashMap<NodeId, RequestTx>>>,
}

impl Default for Router {
  fn default() -> Self {
    Self::new()
  }
}

impl Router {
  pub fn new() -> Self {
    Self {
      targets: Arc::new(Mutex::new(HashMap::new())),
    }
  }

  pub fn add_target(&self, id: NodeId, tx: RequestTx) {
    let mut targets = self.targets.lock().unwrap();
    targets.insert(id, tx);
  }
}

impl RaftNetworkFactory<TypeConfig> for Router {
  type Network = NetworkConnection;

  async fn new_client(&mut self, target: NodeId, _node: &openraft::BasicNode) -> Self::Network {
    let targets = self.targets.clone();
    NetworkConnection { target, targets }
  }
}

pub struct NetworkConnection {
  target: NodeId,
  targets: Arc<Mutex<HashMap<NodeId, RequestTx>>>,
}

impl NetworkConnection {
  async fn send_rpc(
    &self,
    path: &str,
    req: String,
  ) -> Result<
    String,
    RPCError<TypeConfig>,
  > {
    let tx = {
      let targets = self.targets.lock().unwrap();
      targets.get(&self.target).cloned()
    };

    if let Some(tx) = tx {
      let (response_tx, response_rx) = oneshot::channel();

      if tx.send((path.to_string(), req, response_tx)).is_ok() {
        if let Ok(response) = response_rx.await {
          Ok(response)
        } else {
          Err(RPCError::Unreachable(openraft::error::Unreachable::new(
            &"Network unreachable",
          )))
        }
      } else {
        Err(RPCError::Unreachable(openraft::error::Unreachable::new(
          &"Send failed",
        )))
      }
    } else {
      Err(RPCError::Unreachable(openraft::error::Unreachable::new(
        &"No target found",
      )))
    }
  }
}

impl RaftNetwork<TypeConfig> for NetworkConnection {
  async fn append_entries(
    &mut self,
    req: openraft::raft::AppendEntriesRequest<TypeConfig>,
  ) -> Result<
    openraft::raft::AppendEntriesResponse<TypeConfig>,
    RPCError<TypeConfig>,
  > {
    let resp = self
      .send_rpc("/raft/append", serde_json::to_string(&req).unwrap())
      .await?;
    let resp: openraft::raft::AppendEntriesResponse<TypeConfig> = serde_json::from_str(&resp).unwrap();
    Ok(resp)
  }

  async fn install_snapshot(
    &mut self,
    req: openraft::raft::InstallSnapshotRequest<TypeConfig>,
  ) -> Result<
    openraft::raft::InstallSnapshotResponse<TypeConfig>,
    RPCError<TypeConfig, InstallSnapshotError>,
  > {
    let resp = self
      .send_rpc("/raft/snapshot", serde_json::to_string(&req).unwrap())
      .await?;
    let resp: openraft::raft::InstallSnapshotResponse<TypeConfig> =
      serde_json::from_str(&resp).unwrap();
    Ok(resp)
  }

  async fn vote(
    &mut self,
    req: openraft::raft::VoteRequest<TypeConfig>,
  ) -> Result<
    openraft::raft::VoteResponse<TypeConfig>,
    RPCError<TypeConfig>,
  > {
    let resp = self
      .send_rpc("/raft/vote", serde_json::to_string(&req).unwrap())
      .await?;
    let resp: openraft::raft::VoteResponse<TypeConfig> = serde_json::from_str(&resp).unwrap();
    Ok(resp)
  }
}

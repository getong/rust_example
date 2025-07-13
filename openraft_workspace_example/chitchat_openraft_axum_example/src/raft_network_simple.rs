use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use openraft::error::{InstallSnapshotError, RPCError, RaftError, VoteError};
use openraft::{RaftNetwork, RaftNetworkFactory};
use tokio::sync::mpsc;

use crate::raft_types::*;

pub type ResponseTx = tokio::sync::oneshot::Sender<String>;
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

    async fn new_client(&mut self, target: NodeId, node: &openraft::BasicNode) -> Self::Network {
        let targets = self.targets.clone();
        NetworkConnection { target, targets }
    }
}

pub struct NetworkConnection {
    target: NodeId,
    targets: Arc<Mutex<HashMap<NodeId, RequestTx>>>,
}

impl NetworkConnection {
    async fn send_rpc(&self, path: &str, req: String) -> Result<String, RPCError<NodeId, openraft::BasicNode, RaftError<NodeId, openraft::BasicNode>>> {
        let tx = {
            let targets = self.targets.lock().unwrap();
            targets.get(&self.target).cloned()
        };

        if let Some(tx) = tx {
            let (response_tx, response_rx) = tokio::sync::oneshot::channel();
            
            if tx.send((path.to_string(), req, response_tx)).is_ok() {
                if let Ok(response) = response_rx.await {
                    Ok(response)
                } else {
                    Err(RPCError::Unreachable(openraft::error::Unreachable::new(&self.target)))
                }
            } else {
                Err(RPCError::Unreachable(openraft::error::Unreachable::new(&self.target)))
            }
        } else {
            Err(RPCError::Unreachable(openraft::error::Unreachable::new(&self.target)))
        }
    }
}

impl RaftNetwork<TypeConfig> for NetworkConnection {
    async fn append_entries(
        &mut self,
        req: openraft::raft::AppendEntriesRequest<TypeConfig>,
    ) -> Result<openraft::raft::AppendEntriesResponse<NodeId>, RPCError<NodeId, openraft::BasicNode, openraft::error::RaftError<NodeId, openraft::BasicNode>>> {
        let resp = self.send_rpc("/raft/append", serde_json::to_string(&req).unwrap()).await?;
        let resp: openraft::raft::AppendEntriesResponse<NodeId> = serde_json::from_str(&resp).unwrap();
        Ok(resp)
    }

    async fn install_snapshot(
        &mut self,
        req: openraft::raft::InstallSnapshotRequest<TypeConfig>,
    ) -> Result<openraft::raft::InstallSnapshotResponse<NodeId>, RPCError<NodeId, openraft::BasicNode, openraft::error::InstallSnapshotError>> {
        let resp = self.send_rpc("/raft/snapshot", serde_json::to_string(&req).unwrap()).await
            .map_err(|e| match e {
                RPCError::Unreachable(u) => RPCError::Unreachable(u),
                RPCError::Timeout(t) => RPCError::Timeout(t),
                RPCError::Network(n) => RPCError::Network(n),
                RPCError::RemoteError(remote_err) => {
                    // Convert the remote error to InstallSnapshotError 
                    RPCError::RemoteError(InstallSnapshotError::RaftError(remote_err.source))
                }
            })?;
        let resp: openraft::raft::InstallSnapshotResponse<NodeId> = serde_json::from_str(&resp).unwrap();
        Ok(resp)
    }

    async fn vote(
        &mut self,
        req: openraft::raft::VoteRequest<NodeId>,
    ) -> Result<openraft::raft::VoteResponse<NodeId>, RPCError<NodeId, openraft::BasicNode, openraft::error::VoteError>> {
        let resp = self.send_rpc("/raft/vote", serde_json::to_string(&req).unwrap()).await
            .map_err(|e| match e {
                RPCError::Unreachable(u) => RPCError::Unreachable(u),
                RPCError::Timeout(t) => RPCError::Timeout(t),
                RPCError::Network(n) => RPCError::Network(n),
                RPCError::RemoteError(remote_err) => {
                    // Convert the remote error to VoteError
                    RPCError::RemoteError(VoteError::RaftError(remote_err.source))
                }
            })?;
        let resp: openraft::raft::VoteResponse<NodeId> = serde_json::from_str(&resp).unwrap();
        Ok(resp)
    }
}

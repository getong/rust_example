use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use openraft::error::Unreachable;
use tokio::sync::oneshot;
use serde::{Serialize, de::DeserializeOwned};

use crate::raft_types::{NodeId, TypeConfig};

pub type RequestTx = tokio::sync::mpsc::UnboundedSender<(String, String, oneshot::Sender<String>)>;

/// Simulate a network router for inter-node communication
#[derive(Debug, Clone, Default)]
pub struct Router {
    pub targets: Arc<Mutex<BTreeMap<NodeId, RequestTx>>>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            targets: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Register a request handler for a node
    pub fn add_target(&self, node_id: NodeId, tx: RequestTx) {
        let mut targets = self.targets.lock().unwrap();
        targets.insert(node_id, tx);
    }

    /// Remove a node from the router
    pub fn remove_target(&self, node_id: NodeId) {
        let mut targets = self.targets.lock().unwrap();
        targets.remove(&node_id);
    }

    /// Send request to target node and wait for response
    pub async fn send<Req, Resp>(&self, to: NodeId, path: &str, req: Req) -> Result<Resp, Unreachable>
    where
        Req: Serialize,
        Result<Resp, openraft::error::RaftError<TypeConfig>>: DeserializeOwned,
    {
        let (resp_tx, resp_rx) = oneshot::channel();

        let encoded_req = encode(req);
        tracing::debug!("send to: {}, {}, {}", to, path, encoded_req);

        {
            let targets = self.targets.lock().unwrap();
            if let Some(tx) = targets.get(&to) {
                if tx.send((path.to_string(), encoded_req, resp_tx)).is_err() {
                    return Err(Unreachable::new(&format!("Failed to send request to node {}", to)));
                }
            } else {
                return Err(Unreachable::new(&format!("No target found for node {}", to)));
            }
        }

        let resp_str = resp_rx.await.map_err(|e| Unreachable::new(&e))?;
        tracing::debug!("resp from: {}, {}, {}", to, path, resp_str);

        let res = decode::<Result<Resp, openraft::error::RaftError<TypeConfig>>>(&resp_str);
        res.map_err(|e| Unreachable::new(&e))
    }
}

pub fn encode<T: Serialize>(t: T) -> String {
    serde_json::to_string(&t).unwrap()
}

pub fn decode<T: DeserializeOwned>(s: &str) -> T {
    serde_json::from_str(s).unwrap()
}

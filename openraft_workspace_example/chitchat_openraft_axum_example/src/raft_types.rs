use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

pub type NodeId = u64;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RaftRequest {
    Set { key: String, value: String },
}

impl RaftRequest {
    pub fn set(key: impl ToString, value: impl ToString) -> Self {
        Self::Set {
            key: key.to_string(),
            value: value.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RaftResponse {
    pub value: Option<String>,
}

/// Data contained in the Raft state machine.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct StateMachineData {
    pub last_applied: Option<openraft::LogId<NodeId>>,
    pub last_membership: openraft::StoredMembership<NodeId>,
    pub data: BTreeMap<String, String>,
}

openraft::declare_raft_types!(
    /// Declare the type configuration for our K/V store.
    pub TypeConfig:
        D = RaftRequest,
        R = RaftResponse,
        SnapshotData = StateMachineData,
);

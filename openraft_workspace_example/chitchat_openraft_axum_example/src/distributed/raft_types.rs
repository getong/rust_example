//! OpenRaft type configuration based on Stract's implementation
//!
//! This module provides the production-ready OpenRaft types following
//! the exact same pattern used by Stract for distributed consensus.

use std::fmt::Debug;
use std::io::Cursor;

use openraft::TokioRuntime;
use serde::{Deserialize, Serialize};

use crate::distributed::member::NodeId;

/// Basic node information for OpenRaft
#[derive(
    Serialize,
    Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
)]
pub struct BasicNode {
    pub addr: String,
}

impl BasicNode {
    /// Creates a new BasicNode
    pub fn new(addr: impl ToString) -> Self {
        Self {
            addr: addr.to_string(),
        }
    }
}

/// DHT request types for OpenRaft operations (following Stract's pattern)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DhtRequest {
    Put { key: String, value: String },
    Get { key: String },
    Delete { key: String },
    // Batch operations for efficiency
    BatchPut { entries: Vec<(String, String)> },
    BatchDelete { keys: Vec<String> },
}

/// DHT response types for OpenRaft operations (following Stract's pattern)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DhtResponse {
    Put,
    Get { value: Option<String> },
    Delete { existed: bool },
    BatchPut,
    BatchDelete { deleted_count: usize },
    Empty,
}

// Declare the OpenRaft type configuration exactly like Stract does
openraft::declare_raft_types!(
    /// Type configuration for the DHT using OpenRaft (following Stract's pattern)
    pub DhtTypeConfig:
        D = DhtRequest,
        R = DhtResponse,
        NodeId = NodeId,
        Node = BasicNode,
        Entry = openraft::Entry<DhtTypeConfig>,
        SnapshotData = Cursor<Vec<u8>>,
        AsyncRuntime = TokioRuntime,
);

impl From<DhtRequest> for DhtRequest {
    fn from(req: DhtRequest) -> Self {
        req
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_node_creation() {
        let node = BasicNode::new("127.0.0.1:8080");
        assert_eq!(node.addr, "127.0.0.1:8080");
    }

    #[test]
    fn test_request_serialization() {
        let request = DhtRequest::Put {
            key: "test".to_string(),
            value: "value".to_string(),
        };
        
        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: DhtRequest = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(format!("{:?}", request), format!("{:?}", deserialized));
    }
}

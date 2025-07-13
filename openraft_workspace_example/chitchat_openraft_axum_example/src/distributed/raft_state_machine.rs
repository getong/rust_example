//! State machine implementation for OpenRaft based on Stract's pattern
//!
//! This provides the production-ready RaftStateMachine trait implementation
//! for managing distributed state with snapshotting support.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io::Cursor;
use std::sync::Arc;

use openraft::storage::{RaftStateMachine, Snapshot, SnapshotMeta};
use openraft::{Entry, EntryPayload, LogId, RaftTypeConfig, StorageError, StorageIOError};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::raft_types::{DhtTypeConfig, DhtRequest, DhtResponse};
use crate::distributed::member::NodeId;

/// State machine data that can be serialized for snapshots
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DhtStateMachineData {
    /// Key-value storage
    pub data: BTreeMap<String, String>,
    
    /// Last applied log index
    pub last_applied_log: Option<LogId<NodeId>>,
}

/// DHT state machine implementing OpenRaft's RaftStateMachine trait
pub struct DhtStateMachine {
    /// The current state data
    state: Arc<RwLock<DhtStateMachineData>>,
}

impl DhtStateMachine {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(DhtStateMachineData::default())),
        }
    }

    /// Get a value from the state machine
    pub async fn get(&self, key: &str) -> Option<String> {
        let state = self.state.read().await;
        state.data.get(key).cloned()
    }

    /// Put a value in the state machine (used for testing)
    pub async fn put(&self, key: String, value: String) {
        let mut state = self.state.write().await;
        state.data.insert(key, value);
    }

    /// Delete a value from the state machine (used for testing)
    pub async fn delete(&self, key: &str) -> bool {
        let mut state = self.state.write().await;
        state.data.remove(key).is_some()
    }

    /// Get the number of entries in the state machine
    pub async fn len(&self) -> usize {
        let state = self.state.read().await;
        state.data.len()
    }

    /// Check if the state machine is empty
    pub async fn is_empty(&self) -> bool {
        let state = self.state.read().await;
        state.data.is_empty()
    }

    /// Apply a single request to the state machine
    async fn apply_request(&self, request: &DhtRequest) -> DhtResponse {
        let mut state = self.state.write().await;
        
        match request {
            DhtRequest::Put { key, value } => {
                state.data.insert(key.clone(), value.clone());
                DhtResponse::Put
            }
            DhtRequest::Get { key } => {
                let value = state.data.get(key).cloned();
                DhtResponse::Get { value }
            }
            DhtRequest::Delete { key } => {
                let existed = state.data.remove(key).is_some();
                DhtResponse::Delete { existed }
            }
            DhtRequest::BatchPut { entries } => {
                for (key, value) in entries {
                    state.data.insert(key.clone(), value.clone());
                }
                DhtResponse::BatchPut
            }
            DhtRequest::BatchDelete { keys } => {
                let mut deleted_count = 0;
                for key in keys {
                    if state.data.remove(key).is_some() {
                        deleted_count += 1;
                    }
                }
                DhtResponse::BatchDelete { deleted_count }
            }
        }
    }

    /// Create a snapshot of the current state
    async fn create_snapshot(&self, last_applied_log: LogId<NodeId>) -> Result<Cursor<Vec<u8>>, StorageError<NodeId>> {
        let state = self.state.read().await;
        let snapshot_data = DhtStateMachineData {
            data: state.data.clone(),
            last_applied_log: Some(last_applied_log),
        };

        let serialized = serde_json::to_vec(&snapshot_data)
            .map_err(|e| StorageIOError::read_snapshot(None, &e))?;

        Ok(Cursor::new(serialized))
    }

    /// Install a snapshot into the state machine
    async fn install_snapshot(&self, snapshot_data: &[u8]) -> Result<(), StorageError<NodeId>> {
        let snapshot: DhtStateMachineData = serde_json::from_slice(snapshot_data)
            .map_err(|e| StorageIOError::read_snapshot(None, &e))?;

        let mut state = self.state.write().await;
        *state = snapshot;

        Ok(())
    }
}

impl Default for DhtStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DhtStateMachine {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

#[async_trait::async_trait]
impl RaftStateMachine<DhtTypeConfig> for DhtStateMachine {
    type SnapshotBuilder = Self;

    async fn applied_state(
        &mut self,
    ) -> Result<(Option<LogId<NodeId>>, openraft::storage::StateMachineChanges<DhtTypeConfig>), StorageError<NodeId>> {
        let state = self.state.read().await;
        Ok((
            state.last_applied_log,
            openraft::storage::StateMachineChanges::default(),
        ))
    }

    async fn apply<I>(&mut self, entries: I) -> Result<Vec<DhtResponse>, StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry<DhtTypeConfig>> + Send,
        I::IntoIter: Send,
    {
        let mut responses = Vec::new();
        
        for entry in entries {
            let mut state = self.state.write().await;
            state.last_applied_log = Some(entry.log_id);
            drop(state); // Release the lock before applying
            
            match entry.payload {
                EntryPayload::Blank => {
                    responses.push(DhtResponse::Empty);
                }
                EntryPayload::Normal(request) => {
                    let response = self.apply_request(&request).await;
                    responses.push(response);
                }
                EntryPayload::Membership(_) => {
                    responses.push(DhtResponse::Empty);
                }
            }
        }
        
        Ok(responses)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        self.clone()
    }
}

#[async_trait::async_trait]
impl openraft::storage::RaftSnapshotBuilder<DhtTypeConfig> for DhtStateMachine {
    async fn build_snapshot(&mut self) -> Result<Snapshot<DhtTypeConfig>, StorageError<NodeId>> {
        let state = self.state.read().await;
        let last_applied_log = state.last_applied_log;
        drop(state);

        if let Some(last_log_id) = last_applied_log {
            let snapshot_data = self.create_snapshot(last_log_id).await?;
            
            let snapshot_meta = SnapshotMeta {
                last_log_id: Some(last_log_id),
                last_membership: openraft::membership::EffectiveMembership::default(),
                snapshot_id: format!("snapshot-{}", last_log_id.index),
            };

            Ok(Snapshot {
                meta: snapshot_meta,
                snapshot: snapshot_data,
            })
        } else {
            // No logs applied yet, create empty snapshot
            let snapshot_data = Cursor::new(Vec::new());
            let snapshot_meta = SnapshotMeta {
                last_log_id: None,
                last_membership: openraft::membership::EffectiveMembership::default(),
                snapshot_id: "empty-snapshot".to_string(),
            };

            Ok(Snapshot {
                meta: snapshot_meta,
                snapshot: snapshot_data,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_state_machine_operations() {
        let mut sm = DhtStateMachine::new();

        // Test put operation
        sm.put("key1".to_string(), "value1".to_string()).await;
        assert_eq!(sm.get("key1").await, Some("value1".to_string()));

        // Test delete operation
        let existed = sm.delete("key1").await;
        assert!(existed);
        assert_eq!(sm.get("key1").await, None);

        // Test delete non-existent key
        let existed = sm.delete("non-existent").await;
        assert!(!existed);
    }

    #[tokio::test]
    async fn test_apply_request() {
        let sm = DhtStateMachine::new();

        // Test put request
        let put_request = DhtRequest::Put {
            key: "test_key".to_string(),
            value: "test_value".to_string(),
        };
        let response = sm.apply_request(&put_request).await;
        assert!(matches!(response, DhtResponse::Put));

        // Test get request
        let get_request = DhtRequest::Get {
            key: "test_key".to_string(),
        };
        let response = sm.apply_request(&get_request).await;
        assert!(matches!(response, DhtResponse::Get { value: Some(_) }));

        // Test delete request
        let delete_request = DhtRequest::Delete {
            key: "test_key".to_string(),
        };
        let response = sm.apply_request(&delete_request).await;
        assert!(matches!(response, DhtResponse::Delete { existed: true }));
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let sm = DhtStateMachine::new();

        // Test batch put
        let batch_put = DhtRequest::BatchPut {
            entries: vec![
                ("key1".to_string(), "value1".to_string()),
                ("key2".to_string(), "value2".to_string()),
            ],
        };
        let response = sm.apply_request(&batch_put).await;
        assert!(matches!(response, DhtResponse::BatchPut));

        // Verify values were set
        assert_eq!(sm.get("key1").await, Some("value1".to_string()));
        assert_eq!(sm.get("key2").await, Some("value2".to_string()));

        // Test batch delete
        let batch_delete = DhtRequest::BatchDelete {
            keys: vec!["key1".to_string(), "key2".to_string(), "non-existent".to_string()],
        };
        let response = sm.apply_request(&batch_delete).await;
        assert!(matches!(response, DhtResponse::BatchDelete { deleted_count: 2 }));

        // Verify values were deleted
        assert_eq!(sm.get("key1").await, None);
        assert_eq!(sm.get("key2").await, None);
    }
}

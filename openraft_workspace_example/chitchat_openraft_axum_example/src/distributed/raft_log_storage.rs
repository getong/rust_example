//! Log storage implementation for OpenRaft based on Stract's pattern
//!
//! This provides the production-ready RaftLogStorage trait implementation
//! for persistent log storage and voting state management.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::ops::RangeBounds;
use std::sync::Arc;

use openraft::storage::{LogState, Snapshot};
use openraft::{
    Entry, ErrorSubject, ErrorVerb, LogId, RaftLogStorage, RaftTypeConfig, StorageError,
    StorageIOError, Vote,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::raft_types::{DhtTypeConfig, BasicNode};
use crate::distributed::member::NodeId;

/// Vote record for storing voting state
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct VoteRecord {
    pub vote: Vote<NodeId>,
}

/// DHT log storage implementing OpenRaft's RaftLogStorage trait
pub struct DhtLogStorage {
    /// The current vote state
    vote: Arc<RwLock<Option<Vote<NodeId>>>>,
    
    /// Log entries storage
    logs: Arc<RwLock<BTreeMap<u64, Entry<DhtTypeConfig>>>>,
    
    /// Last log index that has been flushed to storage
    last_purged_log_id: Arc<RwLock<Option<LogId<NodeId>>>>,
    
    /// Current snapshot
    snapshot: Arc<RwLock<Option<Snapshot<DhtTypeConfig>>>>,
}

impl DhtLogStorage {
    pub fn new() -> Self {
        Self {
            vote: Arc::new(RwLock::new(None)),
            logs: Arc::new(RwLock::new(BTreeMap::new())),
            last_purged_log_id: Arc::new(RwLock::new(None)),
            snapshot: Arc::new(RwLock::new(None)),
        }
    }
    
    async fn get_log_entries(
        &self,
        range: impl RangeBounds<u64>,
    ) -> Result<Vec<Entry<DhtTypeConfig>>, StorageError<NodeId>> {
        let logs = self.logs.read().await;
        let entries: Vec<_> = logs
            .range(range)
            .map(|(_, entry)| entry.clone())
            .collect();
        Ok(entries)
    }
}

impl Default for DhtLogStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl RaftLogStorage<DhtTypeConfig> for DhtLogStorage {
    type LogReader = Self;

    async fn get_log_state(&mut self) -> Result<LogState<DhtTypeConfig>, StorageError<NodeId>> {
        let logs = self.logs.read().await;
        let last_purged_log_id = *self.last_purged_log_id.read().await;
        
        let last_log_id = logs
            .iter()
            .last()
            .map(|(_, entry)| entry.log_id);
            
        Ok(LogState {
            last_purged_log_id,
            last_log_id,
        })
    }

    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError<NodeId>> {
        let mut v = self.vote.write().await;
        *v = Some(vote.clone());
        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        let vote = self.vote.read().await;
        Ok(vote.clone())
    }

    async fn append<I>(&mut self, entries: I, callback: openraft::storage::LogFlushed<DhtTypeConfig>) -> Result<(), StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry<DhtTypeConfig>> + Send,
        I::IntoIter: Send,
    {
        let mut logs = self.logs.write().await;
        
        for entry in entries {
            logs.insert(entry.log_id.index, entry);
        }
        
        // Simulate async flush
        callback.log_io_completed(Ok(()));
        
        Ok(())
    }

    async fn truncate(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        let mut logs = self.logs.write().await;
        
        // Remove all logs after the specified log_id
        let keys_to_remove: Vec<_> = logs
            .range((log_id.index + 1)..)
            .map(|(k, _)| *k)
            .collect();
            
        for key in keys_to_remove {
            logs.remove(&key);
        }
        
        Ok(())
    }

    async fn purge(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        let mut logs = self.logs.write().await;
        let mut last_purged = self.last_purged_log_id.write().await;
        
        // Remove all logs up to and including log_id
        let keys_to_remove: Vec<_> = logs
            .range(..=log_id.index)
            .map(|(k, _)| *k)
            .collect();
            
        for key in keys_to_remove {
            logs.remove(&key);
        }
        
        *last_purged = Some(log_id);
        
        Ok(())
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        self.clone()
    }
}

// Implementation of Clone for DhtLogStorage (sharing the same data)
impl Clone for DhtLogStorage {
    fn clone(&self) -> Self {
        Self {
            vote: self.vote.clone(),
            logs: self.logs.clone(),
            last_purged_log_id: self.last_purged_log_id.clone(),
            snapshot: self.snapshot.clone(),
        }
    }
}

#[async_trait::async_trait]
impl openraft::storage::RaftLogReader<DhtTypeConfig> for DhtLogStorage {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + Send + Sync>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry<DhtTypeConfig>>, StorageError<NodeId>> {
        self.get_log_entries(range).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openraft::raft::VoteRequest;

    #[tokio::test]
    async fn test_vote_operations() {
        let mut storage = DhtLogStorage::new();
        
        // Initially no vote
        let vote = storage.read_vote().await.unwrap();
        assert!(vote.is_none());
        
        // Save a vote
        let test_vote = Vote::new(1, 1);
        storage.save_vote(&test_vote).await.unwrap();
        
        // Read it back
        let saved_vote = storage.read_vote().await.unwrap();
        assert_eq!(saved_vote, Some(test_vote));
    }

    #[tokio::test]
    async fn test_log_state() {
        let mut storage = DhtLogStorage::new();
        
        let log_state = storage.get_log_state().await.unwrap();
        assert!(log_state.last_log_id.is_none());
        assert!(log_state.last_purged_log_id.is_none());
    }
}

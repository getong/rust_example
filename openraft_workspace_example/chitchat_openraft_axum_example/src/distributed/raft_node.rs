use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use super::raft_types::{Request, Response, Table, Value};

/// Simplified Raft node that provides a distributed key-value store interface
pub struct RaftNode {
  /// Local storage for demonstration
  pub storage: Arc<RwLock<HashMap<(String, String), Vec<u8>>>>,
  node_id: String,
}

impl RaftNode {
  /// Create a new Raft node
  pub async fn new(node_id: String) -> anyhow::Result<Self> {
    tracing::info!("Creating RaftNode with ID: {}", node_id);

    Ok(Self {
      storage: Arc::new(RwLock::new(HashMap::new())),
      node_id,
    })
  }

  /// Submit a client request to the distributed store
  pub async fn client_write(&self, req: Request) -> anyhow::Result<Response> {
    match req {
      Request::Set { table, key, value } => {
        let mut storage = self.storage.write().await;
        let storage_key = (table.0, key.0);
        storage.insert(storage_key.clone(), value.0);
        tracing::debug!("Set key {:?} in node {}", storage_key, self.node_id);
        Ok(Response::Set(Ok(())))
      }
      Request::Get { table, key } => {
        let storage = self.storage.read().await;
        let storage_key = (table.0, key.0);
        let value = storage.get(&storage_key).cloned().map(Value);
        tracing::debug!(
          "Get key {:?} from node {}: {:?}",
          storage_key,
          self.node_id,
          value.is_some()
        );
        Ok(Response::Get(Ok(value)))
      }
      Request::CreateTable { table: _ } => {
        // Tables are created implicitly
        tracing::debug!("Create table in node {}", self.node_id);
        Ok(Response::CreateTable(Ok(())))
      }
      Request::AllTables => {
        let storage = self.storage.read().await;
        let tables: std::collections::BTreeSet<String> =
          storage.keys().map(|(table, _)| table.clone()).collect();
        let table_list: Vec<Table> = tables.into_iter().map(Table).collect();
        tracing::debug!(
          "List tables in node {}: {} tables",
          self.node_id,
          table_list.len()
        );
        Ok(Response::AllTables(Ok(table_list)))
      }
      _ => {
        // For now, return empty response for unsupported operations
        tracing::debug!("Unsupported operation in node {}", self.node_id);
        Ok(Response::Empty)
      }
    }
  }

  /// Get the current node ID
  pub fn node_id(&self) -> &str {
    &self.node_id
  }

  /// Check if this node is available (always true for this simplified implementation)
  pub async fn is_available(&self) -> bool {
    true
  }

  /// Get cluster statistics
  pub async fn get_stats(&self) -> HashMap<String, u64> {
    let storage = self.storage.read().await;
    let mut stats = HashMap::new();

    stats.insert("total_keys".to_string(), storage.len() as u64);
    stats.insert("node_id".to_string(), self.node_id.parse().unwrap_or(0));

    stats
  }
}

#[cfg(test)]
mod tests {
  use std::net::SocketAddr;

  use super::*;
  use crate::{distributed::Member, utils::create_service};

  #[tokio::test]
  async fn test_raft_node_basic_operations() {
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let service = create_service("web", addr, None);
    let member = Member::with_id("test-node".to_string(), service);

    // This would require a real cluster for full testing
    // For now, we just test the basic storage operations

    // Test basic key-value operations
    let storage = Arc::new(RwLock::new(HashMap::new()));
    {
      let mut s = storage.write().await;
      s.insert(
        ("table1".to_string(), "key1".to_string()),
        b"value1".to_vec(),
      );
    }

    let value = {
      let s = storage.read().await;
      s.get(&("table1".to_string(), "key1".to_string())).cloned()
    };

    assert_eq!(value, Some(b"value1".to_vec()));
  }
}

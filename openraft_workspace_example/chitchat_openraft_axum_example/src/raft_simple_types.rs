use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};

// For now, let's create a simplified version that follows the exact pattern
// from the working OpenRaft example

pub type NodeId = u64;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Request {
  Set { key: String, value: String },
}

impl Request {
  pub fn set(key: impl ToString, value: impl ToString) -> Self {
    Self::Set {
      key: key.to_string(),
      value: value.to_string(),
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
  pub value: Option<String>,
}

/// Data contained in the Raft state machine.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct StateMachineData {
  pub last_applied: Option<openraft::LogId<TypeConfig>>,
  pub last_membership: openraft::StoredMembership<TypeConfig>,
  /// Application data.
  pub data: BTreeMap<String, String>,
}

openraft::declare_raft_types!(
    /// Declare the type configuration for example K/V store.
    pub TypeConfig:
        D = Request,
        R = Response,
        SnapshotData = StateMachineData,
);

pub type LogStore = mem_log::LogStore<TypeConfig>;
pub type StateMachineStore = crate::raft_simple_store::StateMachineStore;

/// Function to create and initialize a new raft instance
/// This follows the exact pattern from the working example
pub async fn new_raft(
  node_id: NodeId,
  router: crate::raft_simple_network::Router,
) -> (openraft::Raft<TypeConfig>, Arc<StateMachineStore>) {
  // Create a configuration for the raft instance.
  let config = openraft::Config {
    heartbeat_interval: 500,
    election_timeout_min: 1500,
    election_timeout_max: 3000,
    // Once snapshot is built, delete the logs at once.
    max_in_snapshot_log_to_keep: 0,
    ..Default::default()
  };

  let config = Arc::new(config.validate().unwrap());

  // Create a instance of where the Raft logs will be stored.
  let log_store = LogStore::default();

  // Create a instance of where the state machine data will be stored.
  let state_machine_store = Arc::new(StateMachineStore::default());

  // Create a local raft instance.
  let raft = openraft::Raft::new(
    node_id,
    config,
    router,
    log_store,
    state_machine_store.clone(),
  )
  .await
  .unwrap();

  (raft, state_machine_store)
}

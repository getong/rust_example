use std::{
  collections::BTreeMap,
  fmt::Debug,
  io::Cursor,
  sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
  },
};

use openraft::{
  alias::SnapshotDataOf,
  storage::{RaftStateMachine, Snapshot},
  Entry, EntryPayload, LogId, RaftSnapshotBuilder, SnapshotMeta, StorageError, StoredMembership,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::TypeConfig;

pub type LogStore = mem_log::LogStore<TypeConfig>;

/// Here you will set the types of request that will interact with the raft nodes.
/// For example the `Set` will be used to write data (key and value) to the raft database.
/// The `AddNode` will append a new node to the current existing shared list of nodes.
/// You will want to add any request that can write data in all nodes here.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Request {
  Set { key: String, value: String },
}

/// Here you will defined what type of answer you expect from reading the data of a node.
/// In this example it will return a optional value from a given key in
/// the `Request.Set`.
///
/// TODO: Should we explain how to create multiple `AppDataResponse`?
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
  pub value: Option<String>,
}

#[derive(Debug)]
pub struct StoredSnapshot {
  pub meta: SnapshotMeta<TypeConfig>,

  /// The data of the state machine at the time of this snapshot.
  pub data: Vec<u8>,
}

/// Data contained in the Raft state machine.
///
/// Note that we are using `serde` to serialize the
/// `data`, which has a implementation to be serialized. Note that for this test we set both the key
/// and value as String, but you could set any type of value that has the serialization impl.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct StateMachineData {
  pub last_applied_log: Option<LogId<TypeConfig>>,

  pub last_membership: StoredMembership<TypeConfig>,

  /// Application data.
  pub data: BTreeMap<String, String>,
}

/// Defines a state machine for the Raft cluster. This state machine represents a copy of the
/// data for this node. Additionally, it is responsible for storing the last snapshot of the data.
#[derive(Debug, Default)]
pub struct StateMachineStore {
  /// The Raft state machine.
  pub state_machine: RwLock<StateMachineData>,

  /// Used in identifier for snapshot.
  ///
  /// Note that concurrently created snapshots and snapshots created on different nodes
  /// are not guaranteed to have sequential `snapshot_idx` values, but this does not matter for
  /// correctness.
  snapshot_idx: AtomicU64,

  /// The last received snapshot.
  current_snapshot: RwLock<Option<StoredSnapshot>>,
}

impl RaftSnapshotBuilder<TypeConfig> for Arc<StateMachineStore> {
  #[tracing::instrument(level = "trace", skip(self))]
  async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfig>, StorageError<TypeConfig>> {
    // Serialize the data of the state machine.
    let state_machine = self.state_machine.read().await;
    let data =
      serde_json::to_vec(&state_machine.data).map_err(|e| StorageError::read_state_machine(&e))?;

    let last_applied_log = state_machine.last_applied_log;
    let last_membership = state_machine.last_membership.clone();

    // Lock the current snapshot before releasing the lock on the state machine, to avoid a race
    // condition on the written snapshot
    let mut current_snapshot = self.current_snapshot.write().await;
    drop(state_machine);

    let snapshot_idx = self.snapshot_idx.fetch_add(1, Ordering::Relaxed) + 1;
    let snapshot_id = if let Some(last) = last_applied_log {
      format!(
        "{}-{}-{}",
        last.committed_leader_id(),
        last.index(),
        snapshot_idx
      )
    } else {
      format!("--{}", snapshot_idx)
    };

    let meta = SnapshotMeta {
      last_log_id: last_applied_log,
      last_membership,
      snapshot_id,
    };

    let snapshot = StoredSnapshot {
      meta: meta.clone(),
      data: data.clone(),
    };

    *current_snapshot = Some(snapshot);

    Ok(Snapshot {
      meta,
      snapshot: Cursor::new(data),
    })
  }
}

impl RaftStateMachine<TypeConfig> for Arc<StateMachineStore> {
  type SnapshotBuilder = Self;

  async fn applied_state(
    &mut self,
  ) -> Result<(Option<LogId<TypeConfig>>, StoredMembership<TypeConfig>), StorageError<TypeConfig>>
  {
    let state_machine = self.state_machine.read().await;
    Ok((
      state_machine.last_applied_log,
      state_machine.last_membership.clone(),
    ))
  }

  #[tracing::instrument(level = "trace", skip(self, entries))]
  async fn apply<I>(&mut self, entries: I) -> Result<Vec<Response>, StorageError<TypeConfig>>
  where
    I: IntoIterator<Item = Entry<TypeConfig>> + Send,
  {
    let mut res = Vec::new(); // No `with_capacity`; do not know `len` of iterator

    let mut sm = self.state_machine.write().await;

    for entry in entries {
      tracing::debug!(%entry.log_id, "replicate to sm");

      sm.last_applied_log = Some(entry.log_id);

      match entry.payload {
        EntryPayload::Blank => res.push(Response { value: None }),
        EntryPayload::Normal(ref req) => match req {
          Request::Set { key, value } => {
            sm.data.insert(key.clone(), value.clone());
            res.push(Response {
              value: Some(value.clone()),
            })
          }
        },
        EntryPayload::Membership(ref mem) => {
          sm.last_membership = StoredMembership::new(Some(entry.log_id), mem.clone());
          res.push(Response { value: None })
        }
      };
    }
    Ok(res)
  }

  #[tracing::instrument(level = "trace", skip(self))]
  async fn begin_receiving_snapshot(
    &mut self,
  ) -> Result<SnapshotDataOf<TypeConfig>, StorageError<TypeConfig>> {
    Ok(Cursor::new(Vec::new()))
  }

  #[tracing::instrument(level = "trace", skip(self, snapshot))]
  async fn install_snapshot(
    &mut self,
    meta: &SnapshotMeta<TypeConfig>,
    snapshot: SnapshotDataOf<TypeConfig>,
  ) -> Result<(), StorageError<TypeConfig>> {
    tracing::info!(
      { snapshot_size = snapshot.get_ref().len() },
      "decoding snapshot for installation"
    );

    let new_snapshot = StoredSnapshot {
      meta: meta.clone(),
      data: snapshot.into_inner(),
    };

    // Update the state machine.
    let updated_state_machine_data = serde_json::from_slice(&new_snapshot.data)
      .map_err(|e| StorageError::read_snapshot(Some(new_snapshot.meta.signature()), &e))?;
    let updated_state_machine = StateMachineData {
      last_applied_log: meta.last_log_id,
      last_membership: meta.last_membership.clone(),
      data: updated_state_machine_data,
    };
    let mut state_machine = self.state_machine.write().await;
    *state_machine = updated_state_machine;

    // Lock the current snapshot before releasing the lock on the state machine, to avoid a race
    // condition on the written snapshot
    let mut current_snapshot = self.current_snapshot.write().await;
    drop(state_machine);

    // Update current snapshot.
    *current_snapshot = Some(new_snapshot);
    Ok(())
  }

  #[tracing::instrument(level = "trace", skip(self))]
  async fn get_current_snapshot(
    &mut self,
  ) -> Result<Option<Snapshot<TypeConfig>>, StorageError<TypeConfig>> {
    match &*self.current_snapshot.read().await {
      Some(snapshot) => {
        let data = snapshot.data.clone();
        Ok(Some(Snapshot {
          meta: snapshot.meta.clone(),
          snapshot: Cursor::new(data),
        }))
      }
      None => Ok(None),
    }
  }

  async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
    self.clone()
  }
}

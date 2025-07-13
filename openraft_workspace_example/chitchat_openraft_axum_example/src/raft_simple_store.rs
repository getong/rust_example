use std::{
  sync::{Arc, Mutex},
};

use openraft::{EntryPayload, RaftSnapshotBuilder, storage::RaftStateMachine};

use crate::raft_simple_types::*;

#[derive(Debug)]
pub struct StoredSnapshot {
  pub meta: openraft::SnapshotMeta<TypeConfig>,
  /// The data of the state machine at the time of this snapshot.
  pub data: StateMachineData,
}

/// Defines a state machine for the Raft cluster.
#[derive(Debug, Default)]
pub struct StateMachineStore {
  /// The Raft state machine.
  pub state_machine: Mutex<StateMachineData>,
  snapshot_idx: Mutex<u64>,
  /// The last received snapshot.
  current_snapshot: Mutex<Option<StoredSnapshot>>,
}

impl RaftSnapshotBuilder<TypeConfig> for Arc<StateMachineStore> {
  #[tracing::instrument(level = "trace", skip(self))]
  async fn build_snapshot(
    &mut self,
  ) -> Result<openraft::storage::Snapshot<TypeConfig>, openraft::StorageError<TypeConfig>> {
    let data;
    let last_applied_log;
    let last_membership;

    {
      // Serialize the data of the state machine.
      let state_machine = self.state_machine.lock().unwrap().clone();

      last_applied_log = state_machine.last_applied;
      last_membership = state_machine.last_membership.clone();
      data = state_machine;
    }

    let snapshot_idx = {
      let mut l = self.snapshot_idx.lock().unwrap();
      *l += 1;
      *l
    };

    let snapshot_id = if let Some(last) = last_applied_log {
      format!("{}-{}-{}", last.leader_id, last.index, snapshot_idx)
    } else {
      format!("--{}", snapshot_idx)
    };

    let meta = openraft::SnapshotMeta {
      last_log_id: last_applied_log,
      last_membership,
      snapshot_id,
    };

    let snapshot = StoredSnapshot {
      meta: meta.clone(),
      data: data.clone(),
    };

    {
      let mut current_snapshot = self.current_snapshot.lock().unwrap();
      *current_snapshot = Some(snapshot);
    }

    Ok(openraft::storage::Snapshot {
      meta,
      snapshot: data,
    })
  }
}

impl RaftStateMachine<TypeConfig> for Arc<StateMachineStore> {
  type SnapshotBuilder = Self;

  async fn applied_state(
    &mut self,
  ) -> Result<
    (
      Option<openraft::LogId<TypeConfig>>,
      openraft::StoredMembership<TypeConfig>,
    ),
    openraft::StorageError<TypeConfig>,
  > {
    let state_machine = self.state_machine.lock().unwrap();
    Ok((
      state_machine.last_applied,
      state_machine.last_membership.clone(),
    ))
  }

  #[tracing::instrument(level = "trace", skip(self, entries))]
  async fn apply<I>(&mut self, entries: I) -> Result<Vec<Response>, openraft::StorageError<TypeConfig>>
  where
    I: IntoIterator<Item = openraft::Entry<TypeConfig>>,
  {
    let mut res = Vec::new(); //No `with_capacity`; do not know `len` of iterator

    let mut sm = self.state_machine.lock().unwrap();

    for entry in entries {
      tracing::debug!(%entry.log_id, "replicate to sm");

      sm.last_applied = Some(entry.log_id);

      match entry.payload {
        EntryPayload::Blank => res.push(Response { value: None }),
        EntryPayload::Normal(ref req) => match req {
          Request::Set { key, value, .. } => {
            sm.data.insert(key.clone(), value.clone());
            res.push(Response {
              value: Some(value.clone()),
            })
          }
        },
        EntryPayload::Membership(ref mem) => {
          sm.last_membership = openraft::StoredMembership::new(Some(entry.log_id), mem.clone());
          res.push(Response { value: None })
        }
      };
    }
    Ok(res)
  }

  #[tracing::instrument(level = "trace", skip(self))]
  async fn begin_receiving_snapshot(
    &mut self,
  ) -> Result<StateMachineData, openraft::StorageError<TypeConfig>> {
    Ok(Default::default())
  }

  #[tracing::instrument(level = "trace", skip(self, snapshot))]
  async fn install_snapshot(
    &mut self,
    meta: &openraft::SnapshotMeta<TypeConfig>,
    snapshot: StateMachineData,
  ) -> Result<(), openraft::StorageError<TypeConfig>> {
    tracing::info!("install snapshot");

    let new_snapshot = StoredSnapshot {
      meta: meta.clone(),
      data: snapshot,
    };

    // Update the state machine.
    {
      let updated_state_machine: StateMachineData = new_snapshot.data.clone();
      let mut state_machine = self.state_machine.lock().unwrap();
      *state_machine = updated_state_machine;
    }

    // Update current snapshot.
    let mut current_snapshot = self.current_snapshot.lock().unwrap();
    *current_snapshot = Some(new_snapshot);
    Ok(())
  }

  #[tracing::instrument(level = "trace", skip(self))]
  async fn get_current_snapshot(
    &mut self,
  ) -> Result<Option<openraft::storage::Snapshot<TypeConfig>>, openraft::StorageError<TypeConfig>> {
    match &*self.current_snapshot.lock().unwrap() {
      Some(snapshot) => {
        let data = snapshot.data.clone();
        Ok(Some(openraft::storage::Snapshot {
          meta: snapshot.meta.clone(),
          snapshot: data,
        }))
      }
      None => Ok(None),
    }
  }

  async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
    self.clone()
  }
}

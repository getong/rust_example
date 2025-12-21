use std::{
  collections::BTreeMap,
  io,
  sync::{Arc, Mutex},
};

use futures::{Stream, TryStreamExt};
use openraft::{
  EntryPayload, OptionalSend, RaftSnapshotBuilder, StorageError,
  alias::SnapshotDataOf,
  storage::{EntryResponder, RaftStateMachine, Snapshot},
};

use super::raft_types::{
  Key, LogId, Request, Response, SnapshotMeta, StateMachineData, StoredMembership, Table,
  TypeConfig, UpsertAction, UpsertEnum, Value,
};

#[derive(Debug)]
pub struct StoredSnapshot {
  pub meta: SnapshotMeta,
  /// The data of the state machine at the time of this snapshot.
  pub data: StateMachineData,
}

impl StateMachineData {
  pub fn drop_table(&mut self, table: &Table) {
    let table = self.data.remove(table);
    if let Some(table) = table {
      // drop in background as some tables can be large
      std::thread::spawn(move || {
        drop(table);
      });
    }
  }

  pub fn get(&self, table: &Table, key: &Key) -> Option<Value> {
    self.data.get(table).and_then(|m| m.get(key).cloned())
  }

  pub fn set(&mut self, table: Table, key: Key, value: Value) {
    self.data.entry(table).or_default().insert(key, value);
  }

  pub fn batch_set(&mut self, table: Table, values: Vec<(Key, Value)>) {
    let table_data = self.data.entry(table).or_default();

    for (k, v) in values {
      table_data.insert(k, v);
    }
  }

  pub fn num_keys(&self, table: &Table) -> usize {
    self.data.get(table).map(|m| m.len()).unwrap_or(0)
  }

  pub fn upsert(
    &mut self,
    table: Table,
    upsert_fn: &UpsertEnum,
    key: Key,
    value: Value,
  ) -> UpsertAction {
    let table_data = self.data.entry(table).or_default();

    match table_data.get_mut(&key) {
      Some(old) => {
        let merged = upsert_fn.upsert(old.clone(), value);
        let has_changed = merged != *old;

        *old = merged;

        if has_changed {
          UpsertAction::Merged
        } else {
          UpsertAction::NoChange
        }
      }
      None => {
        table_data.insert(key, value);
        UpsertAction::Inserted
      }
    }
  }

  pub fn batch_upsert(
    &mut self,
    table: Table,
    upsert_fn: &UpsertEnum,
    values: Vec<(Key, Value)>,
  ) -> Vec<(Key, UpsertAction)> {
    let table_data = self.data.entry(table).or_default();
    let mut res = Vec::with_capacity(values.len());

    for (key, value) in values {
      match table_data.get_mut(&key) {
        Some(old) => {
          let merged = upsert_fn.upsert(old.clone(), value);
          let has_changed = merged != *old;

          *old = merged;

          if has_changed {
            res.push((key, UpsertAction::Merged));
          } else {
            res.push((key, UpsertAction::NoChange));
          }
        }
        None => {
          table_data.insert(key.clone(), value);
          res.push((key, UpsertAction::Inserted));
        }
      }
    }

    res
  }

  pub fn clone_table(&mut self, from: &Table, to: Table) {
    let data = self.data.get(from).cloned().unwrap_or_default();
    self.data.insert(to, data);
  }

  pub fn new_table(&mut self, table: Table) {
    self.data.insert(table, BTreeMap::new());
  }

  pub fn tables(&self) -> Vec<Table> {
    self.data.keys().cloned().collect()
  }

  pub fn batch_get(&self, table: &Table, keys: &[Key]) -> Vec<(Key, Value)> {
    match self.data.get(table) {
      None => Vec::new(),
      Some(table) => keys
        .iter()
        .filter_map(|key| table.get(key).map(|value| (key.clone(), value.clone())))
        .collect(),
    }
  }
}

/// Defines a state machine for the Raft cluster. This state machine represents a copy of the
/// data for this node. Additionally, it is responsible for storing the last snapshot of the data.
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
  async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfig>, io::Error> {
    let data;
    let last_applied_log;
    let last_membership;

    {
      // Clone the state machine data.
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

    {
      let mut current_snapshot = self.current_snapshot.lock().unwrap();
      *current_snapshot = Some(snapshot);
    }

    let snapshot_data =
      serde_json::to_vec(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(Snapshot {
      meta,
      snapshot: std::io::Cursor::new(snapshot_data),
    })
  }
}

impl RaftStateMachine<TypeConfig> for Arc<StateMachineStore> {
  type SnapshotBuilder = Self;

  async fn applied_state(&mut self) -> Result<(Option<LogId>, StoredMembership), io::Error> {
    let state_machine = self.state_machine.lock().unwrap();
    Ok((
      state_machine.last_applied,
      state_machine.last_membership.clone(),
    ))
  }

  #[tracing::instrument(level = "trace", skip(self, entries))]
  async fn apply<Strm>(&mut self, mut entries: Strm) -> Result<(), io::Error>
  where
    Strm: Stream<Item = Result<EntryResponder<TypeConfig>, io::Error>> + Unpin + OptionalSend,
  {
    while let Some((entry, responder)) = entries.try_next().await? {
      let mut sm = self.state_machine.lock().unwrap();
      tracing::debug!(%entry.log_id, "replicate to sm");

      sm.last_applied = Some(entry.log_id);

      let response = match entry.payload {
        EntryPayload::Blank => Response::Empty,
        EntryPayload::Normal(ref req) => match req {
          Request::Set { table, key, value } => {
            sm.set(table.clone(), key.clone(), value.clone());
            Response::Set(Ok(()))
          }
          Request::BatchSet { table, values } => {
            sm.batch_set(table.clone(), values.clone());
            Response::Set(Ok(()))
          }
          Request::Get { table, key } => {
            let value = sm.get(table, key);
            Response::Get(Ok(value))
          }
          Request::BatchGet { table, keys } => {
            let values = sm.batch_get(table, keys);
            Response::BatchGet(Ok(values))
          }
          Request::Upsert {
            table,
            key,
            value,
            upsert_fn,
          } => Response::Upsert(Ok(sm.upsert(
            table.clone(),
            upsert_fn,
            key.clone(),
            value.clone(),
          ))),
          Request::BatchUpsert {
            table,
            upsert_fn,
            values,
          } => Response::BatchUpsert(Ok(sm.batch_upsert(
            table.clone(),
            upsert_fn,
            values.clone(),
          ))),
          Request::CreateTable { table } => {
            sm.new_table(table.clone());
            Response::CreateTable(Ok(()))
          }
          Request::DropTable { table } => {
            sm.drop_table(table);
            Response::DropTable(Ok(()))
          }
          Request::AllTables => Response::AllTables(Ok(sm.tables())),
          Request::CloneTable { from, to } => {
            sm.clone_table(from, to.clone());
            Response::CloneTable(Ok(()))
          }
        },
        EntryPayload::Membership(ref mem) => {
          sm.last_membership = StoredMembership::new(Some(entry.log_id), mem.clone());
          Response::Empty
        }
      };
      drop(sm);

      if let Some(responder) = responder {
        responder.send(response);
      }
    }
    Ok(())
  }

  #[tracing::instrument(level = "trace", skip(self))]
  async fn begin_receiving_snapshot(&mut self) -> Result<SnapshotDataOf<TypeConfig>, io::Error> {
    Ok(std::io::Cursor::new(Vec::new()))
  }
  #[tracing::instrument(level = "trace", skip(self, snapshot))]
  async fn install_snapshot(
    &mut self,
    meta: &SnapshotMeta,
    snapshot: SnapshotDataOf<TypeConfig>,
  ) -> Result<(), io::Error> {
    tracing::info!("install snapshot");

    // Deserialize the snapshot data
    let mut snapshot_data: StateMachineData = serde_json::from_slice(snapshot.get_ref())
      .map_err(|e| StorageError::read_snapshot(Some(meta.signature()), &e))?;
    snapshot_data.last_applied = meta.last_log_id;
    snapshot_data.last_membership = meta.last_membership.clone();

    let new_snapshot = StoredSnapshot {
      meta: meta.clone(),
      data: snapshot_data.clone(),
    };

    // Update the state machine.
    {
      let mut state_machine = self.state_machine.lock().unwrap();
      *state_machine = snapshot_data;
    }

    // Update current snapshot.
    let mut current_snapshot = self.current_snapshot.lock().unwrap();
    *current_snapshot = Some(new_snapshot);
    Ok(())
  }
  #[tracing::instrument(level = "trace", skip(self))]
  async fn get_current_snapshot(&mut self) -> Result<Option<Snapshot<TypeConfig>>, io::Error> {
    match &*self.current_snapshot.lock().unwrap() {
      Some(snapshot) => {
        let data = serde_json::to_vec(&snapshot.data)
          .map_err(|e| StorageError::read_snapshot(Some(snapshot.meta.signature()), &e))?;
        Ok(Some(Snapshot {
          meta: snapshot.meta.clone(),
          snapshot: std::io::Cursor::new(data),
        }))
      }
      None => Ok(None),
    }
  }

  async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
    self.clone()
  }
}

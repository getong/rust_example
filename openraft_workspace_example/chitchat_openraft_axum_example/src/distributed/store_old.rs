use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use openraft::storage::RaftStateMachine;
use openraft::{EntryPayload, RaftSnapshotBuilder};

use super::raft_types::{
    Entry, Key, LogId, Request, Response, Snapshot, SnapshotMeta, 
    StateMachineData, StorageError, StoredMembership, Table, TypeConfig, UpsertAction, 
    UpsertEnum, Value
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
    async fn build_snapshot(&mut self) -> Result<Snapshot, StorageError> {
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
            format!("{}-{}-{}", last.committed_leader_id(), last.index(), snapshot_idx)
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

        Ok(Snapshot { 
            meta, 
            snapshot: data 
        })
    }
}

impl RaftStateMachine<TypeConfig> for Arc<StateMachineStore> {
    type SnapshotBuilder = Self;

    async fn applied_state(&mut self) -> Result<(Option<LogId>, StoredMembership), StorageError> {
        let state_machine = self.state_machine.lock().unwrap();
        Ok((state_machine.last_applied, state_machine.last_membership.clone()))
    }

    #[tracing::instrument(level = "trace", skip(self, entries))]
    async fn apply<I>(&mut self, entries: I) -> Result<Vec<Response>, StorageError>
    where 
        I: IntoIterator<Item = Entry>
    {
        let mut res = Vec::new();
        let mut sm = self.state_machine.lock().unwrap();

        for entry in entries {
            tracing::debug!(%entry.log_id, "replicate to sm");

            sm.last_applied = Some(entry.log_id);

            match entry.payload {
                EntryPayload::Blank => res.push(Response::Empty),
                EntryPayload::Normal(ref req) => match req {
                    Request::Set { table, key, value } => {
                        sm.set(table.clone(), key.clone(), value.clone());
                        res.push(Response::Set(Ok(())))
                    }
                    Request::BatchSet { table, values } => {
                        sm.batch_set(table.clone(), values.clone());
                        res.push(Response::Set(Ok(())))
                    }
                    Request::Get { table, key } => {
                        let value = sm.get(table, key);
                        res.push(Response::Get(Ok(value)))
                    }
                    Request::BatchGet { table, keys } => {
                        let values = sm.batch_get(table, keys);
                        res.push(Response::BatchGet(Ok(values)))
                    }
                    Request::Upsert {
                        table,
                        key,
                        value,
                        upsert_fn,
                    } => res.push(Response::Upsert(Ok(sm.upsert(
                        table.clone(),
                        upsert_fn,
                        key.clone(),
                        value.clone(),
                    )))),
                    Request::BatchUpsert {
                        table,
                        upsert_fn,
                        values,
                    } => res.push(Response::BatchUpsert(Ok(sm.batch_upsert(
                        table.clone(),
                        upsert_fn,
                        values.clone(),
                    )))),
                    Request::CreateTable { table } => {
                        sm.new_table(table.clone());
                        res.push(Response::CreateTable(Ok(())))
                    }
                    Request::DropTable { table } => {
                        sm.drop_table(table);
                        res.push(Response::DropTable(Ok(())))
                    }
                    Request::AllTables => {
                        res.push(Response::AllTables(Ok(sm.tables())))
                    }
                    Request::CloneTable { from, to } => {
                        sm.clone_table(from, to.clone());
                        res.push(Response::CloneTable(Ok(())))
                    }
                },
                EntryPayload::Membership(ref mem) => {
                    sm.last_membership = StoredMembership::new(Some(entry.log_id), mem.clone());
                    res.push(Response::Empty)
                }
            };
        }
        Ok(res)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn begin_receiving_snapshot(&mut self) -> Result<StateMachineData, StorageError> {
        Ok(Default::default())
    }

    #[tracing::instrument(level = "trace", skip(self, snapshot))]
    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta,
        snapshot: StateMachineData,
    ) -> Result<(), StorageError> {
        tracing::info!("install snapshot");

        let new_snapshot = StoredSnapshot {
            meta: meta.clone(),
            data: snapshot.clone(),
        };

        // Update the state machine.
        {
            let mut state_machine = self.state_machine.lock().unwrap();
            *state_machine = snapshot;
        }

        // Update current snapshot.
        let mut current_snapshot = self.current_snapshot.lock().unwrap();
        *current_snapshot = Some(new_snapshot);
        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn get_current_snapshot(&mut self) -> Result<Option<Snapshot>, StorageError> {
        match &*self.current_snapshot.lock().unwrap() {
            Some(snapshot) => {
                let data = snapshot.data.clone();
                Ok(Some(Snapshot {
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

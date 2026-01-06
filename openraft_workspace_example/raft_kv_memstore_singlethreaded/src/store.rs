use std::{
  collections::BTreeMap,
  fmt,
  fmt::Debug,
  io,
  io::Cursor,
  ops::RangeBounds,
  sync::{Arc, Mutex},
};

use futures::{Stream, TryStreamExt};
use openraft::{
  OptionalSend, RaftLogReader, RaftSnapshotBuilder,
  storage::{EntryResponder, RaftLogStorage, RaftStateMachine},
};
use serde::{Deserialize, Serialize};

use crate::{TypeConfig, typ::*};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Request {
  Set { key: String, value: String },
}

impl fmt::Display for Request {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Request::Set { key, value, .. } => write!(f, "Set {{ key: {}, value: {} }}", key, value),
    }
  }
}

impl Request {
  pub fn set(key: impl ToString, value: impl ToString) -> Self {
    Self::Set {
      key: key.to_string(),
      value: value.to_string(),
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::store::Request;

  #[test]
  fn test_serde() {
    let a = Request::Set {
      key: "foo".to_string(),
      value: "bar".to_string(),
    };

    let b = serde_json::to_string(&a).unwrap();
    println!("{}", b);
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
  pub value: Option<String>,
}

#[derive(Debug)]
pub struct StoredSnapshot {
  pub meta: SnapshotMeta,

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
  pub last_applied: Option<LogId>,

  pub last_membership: StoredMembership,

  /// Application data.
  pub data: BTreeMap<String, String>,
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

#[derive(Debug, Default)]
pub struct LogStore {
  last_purged_log_id: Mutex<Option<LogId>>,

  /// The Raft log.
  log: Mutex<BTreeMap<u64, Entry>>,

  committed: Mutex<Option<LogId>>,

  /// The current granted vote.
  vote: Mutex<Option<Vote>>,
}

impl RaftLogReader<TypeConfig> for Arc<LogStore> {
  async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug>(
    &mut self,
    range: RB,
  ) -> Result<Vec<Entry>, io::Error> {
    let log = self.log.lock().unwrap();
    let response = log
      .range(range.clone())
      .map(|(_, val)| val.clone())
      .collect::<Vec<_>>();
    Ok(response)
  }

  async fn read_vote(&mut self) -> Result<Option<Vote>, io::Error> {
    Ok(*self.vote.lock().unwrap())
  }
}

impl RaftSnapshotBuilder<TypeConfig> for Arc<StateMachineStore> {
  #[tracing::instrument(level = "trace", skip(self))]
  async fn build_snapshot(&mut self) -> Result<Snapshot, io::Error> {
    let data;
    let last_applied_log;
    let last_membership;

    {
      // Serialize the data of the state machine.
      let state_machine = self.state_machine.lock().unwrap();
      data = serde_json::to_vec(&*state_machine)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

      last_applied_log = state_machine.last_applied;
      last_membership = state_machine.last_membership.clone();
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

    Ok(Snapshot {
      meta,
      snapshot: Cursor::new(data),
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
      tracing::debug!(%entry.log_id, "replicate to sm");

      let mut sm = self.state_machine.lock().unwrap();

      sm.last_applied = Some(entry.log_id);

      let response = match entry.payload {
        EntryPayload::Blank => Response { value: None },
        EntryPayload::Normal(ref req) => match req {
          Request::Set { key, value, .. } => {
            sm.data.insert(key.clone(), value.clone());
            Response {
              value: Some(value.clone()),
            }
          }
        },
        EntryPayload::Membership(ref mem) => {
          sm.last_membership = StoredMembership::new(Some(entry.log_id), mem.clone());
          Response { value: None }
        }
      };

      if let Some(responder) = responder {
        responder.send(response);
      }
    }
    Ok(())
  }

  #[tracing::instrument(level = "trace", skip(self))]
  async fn begin_receiving_snapshot(&mut self) -> Result<SnapshotData, io::Error> {
    Ok(Cursor::new(Vec::new()))
  }

  #[tracing::instrument(level = "trace", skip(self, snapshot))]
  async fn install_snapshot(
    &mut self,
    meta: &SnapshotMeta,
    snapshot: SnapshotData,
  ) -> Result<(), io::Error> {
    tracing::info!(
      { snapshot_size = snapshot.get_ref().len() },
      "decoding snapshot for installation"
    );

    let new_snapshot = StoredSnapshot {
      meta: meta.clone(),
      data: snapshot.into_inner(),
    };

    // Update the state machine.
    {
      let updated_state_machine: StateMachineData = serde_json::from_slice(&new_snapshot.data)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
      let mut state_machine = self.state_machine.lock().unwrap();
      *state_machine = updated_state_machine;
    }

    // Update current snapshot.
    let mut current_snapshot = self.current_snapshot.lock().unwrap();
    *current_snapshot = Some(new_snapshot);
    Ok(())
  }

  #[tracing::instrument(level = "trace", skip(self))]
  async fn get_current_snapshot(&mut self) -> Result<Option<Snapshot>, io::Error> {
    let current_snapshot = self.current_snapshot.lock().unwrap();
    match current_snapshot.as_ref() {
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

impl RaftLogStorage<TypeConfig> for Arc<LogStore> {
  type LogReader = Self;

  async fn get_log_state(&mut self) -> Result<LogState, io::Error> {
    let last = {
      let log = self.log.lock().unwrap();
      log.iter().next_back().map(|(_, ent)| ent.log_id)
    };

    let last_purged = *self.last_purged_log_id.lock().unwrap();

    let last = match last {
      None => last_purged,
      Some(x) => Some(x),
    };

    Ok(LogState {
      last_purged_log_id: last_purged,
      last_log_id: last,
    })
  }

  async fn save_committed(&mut self, committed: Option<LogId>) -> Result<(), io::Error> {
    let mut c = self.committed.lock().unwrap();
    *c = committed;
    Ok(())
  }

  async fn read_committed(&mut self) -> Result<Option<LogId>, io::Error> {
    let committed = self.committed.lock().unwrap();
    Ok(*committed)
  }

  #[tracing::instrument(level = "trace", skip(self))]
  async fn save_vote(&mut self, vote: &Vote) -> Result<(), io::Error> {
    let mut v = self.vote.lock().unwrap();
    *v = Some(*vote);
    Ok(())
  }

  #[tracing::instrument(level = "trace", skip(self, entries, callback))]
  async fn append<I>(&mut self, entries: I, callback: IOFlushed) -> Result<(), io::Error>
  where
    I: IntoIterator<Item = Entry>,
  {
    // Simple implementation that calls the flush-before-return `append_to_log`.
    {
      let mut log = self.log.lock().unwrap();
      for entry in entries {
        log.insert(entry.log_id.index(), entry);
      }
    }
    callback.io_completed(Ok(()));

    Ok(())
  }

  #[tracing::instrument(level = "debug", skip(self))]
  async fn truncate_after(&mut self, last_log_id: Option<LogId>) -> Result<(), io::Error> {
    tracing::debug!("truncate_after: ({:?}, +oo)", last_log_id);

    let start_index = match last_log_id {
      Some(log_id) => log_id.index() + 1,
      None => 0,
    };

    let mut log = self.log.lock().unwrap();
    let keys = log
      .range(start_index ..)
      .map(|(k, _v)| *k)
      .collect::<Vec<_>>();
    for key in keys {
      log.remove(&key);
    }

    Ok(())
  }

  #[tracing::instrument(level = "debug", skip(self))]
  async fn purge(&mut self, log_id: LogId) -> Result<(), io::Error> {
    tracing::debug!("delete_log: (-oo, {:?}]", log_id);

    {
      let mut ld = self.last_purged_log_id.lock().unwrap();
      assert!(*ld <= Some(log_id));
      *ld = Some(log_id);
    }

    {
      let mut log = self.log.lock().unwrap();

      let keys = log
        .range(..= log_id.index())
        .map(|(k, _v)| *k)
        .collect::<Vec<_>>();
      for key in keys {
        log.remove(&key);
      }
    }

    Ok(())
  }

  async fn get_log_reader(&mut self) -> Self::LogReader {
    self.clone()
  }
}

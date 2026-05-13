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
  EntryPayload, LogState, OptionalSend, RaftLogReader, RaftSnapshotBuilder,
  alias::LogIdOf,
  alias::SnapshotDataOf,
  alias::SnapshotMetaOf,
  alias::SnapshotOf,
  alias::StoredMembershipOf,
  entry::RaftEntry,
  storage::{EntryResponder, IOFlushed, RaftLogStorage, RaftStateMachine},
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::node::RaftTypeConfig;

/// Here you will set the types of request that will interact with the raft nodes.
/// For example, the `Set` will be used to write data (key and value) to the raft database.
/// The `AddNode` will append a new node to the current existing shared list of nodes.
/// You will want to add any request that can write data in all nodes here.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RaftRequest {
  Set { key: String, value: String },
}

impl fmt::Display for RaftRequest {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      RaftRequest::Set { key, value } => {
        write!(f, "Set {{ key: {}, value_len: {} }}", key, value.len())
      }
    }
  }
}

/// Here you will defined what type of answer you expect from reading the data of a node.
/// In this example it will return a optional value from a given key in
/// the `RaftRequest.Set`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RaftResponse {
  pub value: Option<String>,
}

#[derive(Debug)]
pub struct RaftSnapshot {
  pub meta: SnapshotMetaOf<RaftTypeConfig>,

  /// The data of the state machine at the time of this snapshot.
  pub data: Vec<u8>,
}

/// Here defines a state machine of the raft, this state represents a copy of the data
/// between each node. Note that we are using `serde` to serialize the `data`, which has
/// a implementation to be serialized. Note that for this test we set both the key and
/// value as String, but you could set any type of value that has the serialization impl.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct RaftStateMachineData {
  pub last_applied_log: Option<LogIdOf<RaftTypeConfig>>,

  // TODO: it should not be Option.
  pub last_membership: StoredMembershipOf<RaftTypeConfig>,

  /// Application data.
  pub data: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
pub struct RaftStore {
  last_purged_log_id: RwLock<Option<LogIdOf<RaftTypeConfig>>>,

  /// The Raft log.
  log: RwLock<BTreeMap<u64, <RaftTypeConfig as openraft::RaftTypeConfig>::Entry>>,

  /// The Raft state machine.
  pub state_machine: RwLock<RaftStateMachineData>,

  /// The current granted vote.
  vote: RwLock<Option<<RaftTypeConfig as openraft::RaftTypeConfig>::Vote>>,

  snapshot_idx: Arc<Mutex<u64>>,

  current_snapshot: RwLock<Option<RaftSnapshot>>,
}

impl RaftLogStorage<RaftTypeConfig> for Arc<RaftStore> {
  type LogReader = Self;

  async fn get_log_state(&mut self) -> Result<LogState<RaftTypeConfig>, io::Error> {
    let log = self.log.read().await;
    let last_log_id = log.iter().rev().next().map(|(_, ent)| ent.log_id());
    let last_purged_log_id = *self.last_purged_log_id.read().await;

    Ok(LogState {
      last_purged_log_id,
      last_log_id: last_log_id.or(last_purged_log_id),
    })
  }

  async fn get_log_reader(&mut self) -> Self::LogReader {
    self.clone()
  }

  #[tracing::instrument(level = "trace", skip(self, entries, callback))]
  async fn append<I>(
    &mut self,
    entries: I,
    callback: IOFlushed<RaftTypeConfig>,
  ) -> Result<(), io::Error>
  where
    I: IntoIterator<Item = <RaftTypeConfig as openraft::RaftTypeConfig>::Entry> + Send,
    I::IntoIter: Send,
  {
    let mut log = self.log.write().await;
    for entry in entries {
      log.insert(entry.log_id().index, entry);
    }
    callback.io_completed(Ok(()));
    Ok(())
  }

  #[tracing::instrument(level = "debug", skip(self))]
  async fn truncate_after(
    &mut self,
    log_id: Option<LogIdOf<RaftTypeConfig>>,
  ) -> Result<(), io::Error> {
    tracing::debug!("truncate_after: ({:?}, +oo)", log_id);

    let start_index = match log_id {
      Some(id) => id.index + 1,
      None => 0,
    };

    let mut log = self.log.write().await;
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
  async fn purge(&mut self, log_id: LogIdOf<RaftTypeConfig>) -> Result<(), io::Error> {
    tracing::debug!("purge_log: [0, {:?}]", log_id);

    {
      let mut ld = self.last_purged_log_id.write().await;
      assert!(*ld <= Some(log_id));
      *ld = Some(log_id);
    }

    {
      let mut log = self.log.write().await;
      let keys = log
        .range(..= log_id.index)
        .map(|(k, _v)| *k)
        .collect::<Vec<_>>();
      for key in keys {
        log.remove(&key);
      }
    }

    Ok(())
  }

  async fn save_vote(&mut self, vote: &<RaftTypeConfig as openraft::RaftTypeConfig>::Vote) -> Result<(), io::Error> {
    let mut v = self.vote.write().await;
    *v = Some(*vote);
    Ok(())
  }
}

impl RaftLogReader<RaftTypeConfig> for Arc<RaftStore> {
  async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + Send>(
    &mut self,
    range: RB,
  ) -> Result<Vec<<RaftTypeConfig as openraft::RaftTypeConfig>::Entry>, io::Error> {
    let log = self.log.read().await;
    let response = log
      .range(range.clone())
      .map(|(_, val)| val.clone())
      .collect::<Vec<_>>();
    Ok(response)
  }

  async fn read_vote(&mut self) -> Result<Option<<RaftTypeConfig as openraft::RaftTypeConfig>::Vote>, io::Error> {
    Ok(*self.vote.read().await)
  }
}

impl RaftSnapshotBuilder<RaftTypeConfig> for Arc<RaftStore> {
  #[tracing::instrument(level = "trace", skip(self))]
  async fn build_snapshot(&mut self) -> Result<SnapshotOf<RaftTypeConfig>, io::Error> {
    let data;
    let last_applied_log;
    let last_membership;

    {
      // Serialize the data of the state machine.
      let state_machine = self.state_machine.read().await;
      data = serde_json::to_vec(&*state_machine)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

      last_applied_log = state_machine.last_applied_log;
      last_membership = state_machine.last_membership.clone();
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

    let meta = SnapshotMetaOf::<RaftTypeConfig> {
      last_log_id: last_applied_log,
      last_membership,
      snapshot_id,
    };

    let snapshot = RaftSnapshot {
      meta: meta.clone(),
      data: data.clone(),
    };

    {
      let mut current_snapshot = self.current_snapshot.write().await;
      *current_snapshot = Some(snapshot);
    }

    Ok(SnapshotOf::<RaftTypeConfig> {
      meta,
      snapshot: Cursor::new(data),
    })
  }
}

// Simple RaftStateMachine implementation
impl RaftStateMachine<RaftTypeConfig> for Arc<RaftStore> {
  type SnapshotBuilder = Self;

  async fn applied_state(
    &mut self,
  ) -> Result<
    (
      Option<LogIdOf<RaftTypeConfig>>,
      StoredMembershipOf<RaftTypeConfig>,
    ),
    io::Error,
  > {
    let sm = self.state_machine.read().await;
    Ok((sm.last_applied_log, sm.last_membership.clone()))
  }

  async fn apply<Strm>(&mut self, mut entries: Strm) -> Result<(), io::Error>
  where
    Strm: Stream<Item = Result<EntryResponder<RaftTypeConfig>, io::Error>> + Unpin + OptionalSend,
  {
    while let Some((entry, responder)) = entries.try_next().await? {
      let mut sm = self.state_machine.write().await;
      sm.last_applied_log = Some(entry.log_id());

      let response = match entry.payload {
        EntryPayload::Blank => RaftResponse { value: None },
        EntryPayload::Normal(ref req) => match req {
          RaftRequest::Set { key, value } => {
            sm.data.insert(key.clone(), value.clone());
            RaftResponse {
              value: Some(value.clone()),
            }
          }
        },
        EntryPayload::Membership(ref mem) => {
          sm.last_membership = StoredMembershipOf::<RaftTypeConfig>::new(Some(entry.log_id()), mem.clone());
          RaftResponse { value: None }
        }
      };
      drop(sm);

      if let Some(responder) = responder {
        responder.send(response);
      }
    }
    Ok(())
  }

  async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
    self.clone()
  }

  async fn begin_receiving_snapshot(
    &mut self,
  ) -> Result<SnapshotDataOf<RaftTypeConfig>, io::Error> {
    Ok(Cursor::new(Vec::new()))
  }

  async fn install_snapshot(
    &mut self,
    meta: &SnapshotMetaOf<RaftTypeConfig>,
    snapshot: SnapshotDataOf<RaftTypeConfig>,
  ) -> Result<(), io::Error> {
    // For simplicity, just update the current snapshot
    let data = snapshot.into_inner();
    let new_snapshot = RaftSnapshot {
      meta: meta.clone(),
      data,
    };

    let mut current_snapshot = self.current_snapshot.write().await;
    *current_snapshot = Some(new_snapshot);
    Ok(())
  }

  async fn get_current_snapshot(
    &mut self,
  ) -> Result<Option<SnapshotOf<RaftTypeConfig>>, io::Error> {
    match &*self.current_snapshot.read().await {
      Some(snapshot) => {
        let data = snapshot.data.clone();
        Ok(Some(SnapshotOf::<RaftTypeConfig> {
          meta: snapshot.meta.clone(),
          snapshot: Cursor::new(data),
        }))
      }
      None => Ok(None),
    }
  }
}

// Remove the RaftStorage implementation for now since the API has changed
// We'll need to implement the newer storage API

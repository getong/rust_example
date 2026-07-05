use std::{fmt, fmt::Debug, io, marker::PhantomData, ops::RangeBounds};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use fjall::{Database, Keyspace, KeyspaceCreateOptions, PersistMode};
use openraft::{
  LogState, OptionalSend, RaftLogReader, RaftTypeConfig,
  alias::{EntryOf, LogIdOf, VoteOf},
  entry::RaftEntry,
  storage::{IOFlushed, RaftLogStorage},
  type_config::TypeConfigExt,
};

#[derive(Clone)]
pub struct FjallLogStore<C>
where
  C: RaftTypeConfig,
{
  db: Database,
  meta: Keyspace,
  logs: Keyspace,
  _p: PhantomData<C>,
}

impl<C> Debug for FjallLogStore<C>
where
  C: RaftTypeConfig,
{
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("FjallLogStore").finish_non_exhaustive()
  }
}

impl<C> FjallLogStore<C>
where
  C: RaftTypeConfig,
{
  pub fn new(db: Database) -> Result<Self, io::Error> {
    let meta = db
      .keyspace("meta", KeyspaceCreateOptions::default)
      .map_err(read_logs_err)?;
    let logs = db
      .keyspace("logs", KeyspaceCreateOptions::default)
      .map_err(read_logs_err)?;

    Ok(Self {
      db,
      meta,
      logs,
      _p: PhantomData,
    })
  }

  /// Get a store metadata.
  ///
  /// It returns `None` if the store does not have such a metadata stored.
  fn get_meta<M: StoreMeta<C>>(&self) -> Result<Option<M::Value>, io::Error> {
    let Some(bytes) = self.meta.get(M::KEY).map_err(read_logs_err)? else {
      return Ok(None);
    };

    let t = sonic_rs::from_slice(bytes.as_ref())
      .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(Some(t))
  }

  /// Save a store metadata.
  fn put_meta<M: StoreMeta<C>>(&self, value: &M::Value) -> Result<(), io::Error> {
    let value =
      sonic_rs::to_vec(value).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    self.meta.insert(M::KEY, value).map_err(read_logs_err)
  }

  fn remove_meta<M: StoreMeta<C>>(&self) -> Result<(), io::Error> {
    self.meta.remove(M::KEY).map_err(read_logs_err)
  }

  async fn persist_sync(&self) -> Result<(), io::Error> {
    let db = self.db.clone();
    C::spawn_blocking(move || {
      db.persist(PersistMode::SyncAll)
        .map_err(|e| io::Error::other(e.to_string()))
    })
    .await??;

    Ok(())
  }

  fn remove_logs_from(&self, start_index: u64) -> Result<(), io::Error> {
    let keys = self.collect_log_keys(id_to_bin(start_index) ..)?;
    self.remove_log_keys(keys)
  }

  fn purge_logs_through(&self, log_id: &LogIdOf<C>) -> Result<(), io::Error> {
    let keys = self.collect_log_keys(id_to_bin(0) ..= id_to_bin(log_id.index()))?;
    let mut batch = self.db.batch();
    let value =
      sonic_rs::to_vec(log_id).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    batch.insert(&self.meta, <meta::LastPurged as StoreMeta<C>>::KEY, value);

    for key in keys {
      batch.remove(&self.logs, key);
    }

    batch.commit().map_err(read_logs_err)
  }

  fn collect_log_keys<R>(&self, range: R) -> Result<Vec<Vec<u8>>, io::Error>
  where
    R: RangeBounds<Vec<u8>>,
  {
    self
      .logs
      .range(range)
      .map(|item| {
        let (key, _) = item.into_inner().map_err(read_logs_err)?;
        Ok(key.as_ref().to_vec())
      })
      .collect()
  }

  fn remove_log_keys(&self, keys: Vec<Vec<u8>>) -> Result<(), io::Error> {
    if keys.is_empty() {
      return Ok(());
    }

    let mut batch = self.db.batch();
    for key in keys {
      batch.remove(&self.logs, key);
    }

    batch.commit().map_err(read_logs_err)
  }
}

impl<C> RaftLogReader<C> for FjallLogStore<C>
where
  C: RaftTypeConfig,
{
  async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
    &mut self,
    range: RB,
  ) -> Result<Vec<C::Entry>, io::Error> {
    let start = match range.start_bound() {
      std::ops::Bound::Included(x) => id_to_bin(*x),
      std::ops::Bound::Excluded(x) => {
        let Some(start) = x.checked_add(1) else {
          return Ok(Vec::new());
        };
        id_to_bin(start)
      }
      std::ops::Bound::Unbounded => id_to_bin(0),
    };

    let mut res = Vec::new();

    for item in self.logs.range(start ..) {
      let (id, val) = item.into_inner().map_err(read_logs_err)?;

      let id = bin_to_id(id.as_ref());
      if !range.contains(&id) {
        break;
      }

      let entry: EntryOf<C> = sonic_rs::from_slice(val.as_ref()).map_err(read_logs_err)?;

      assert_eq!(id, entry.index());

      res.push(entry);
    }
    Ok(res)
  }

  async fn read_vote(&mut self) -> Result<Option<VoteOf<C>>, io::Error> {
    self.get_meta::<meta::Vote>()
  }
}

impl<C> RaftLogStorage<C> for FjallLogStore<C>
where
  C: RaftTypeConfig,
{
  type LogReader = Self;

  async fn get_log_state(&mut self) -> Result<LogState<C>, io::Error> {
    let last = self.logs.iter().next_back();

    let last_log_id = match last {
      None => None,
      Some(item) => {
        let (_log_index, entry_bytes) = item.into_inner().map_err(read_logs_err)?;
        let ent =
          sonic_rs::from_slice::<EntryOf<C>>(entry_bytes.as_ref()).map_err(read_logs_err)?;
        Some(ent.log_id())
      }
    };

    let last_purged_log_id = self.get_meta::<meta::LastPurged>()?;

    let last_log_id = match last_log_id {
      None => last_purged_log_id.clone(),
      Some(x) => Some(x),
    };

    Ok(LogState {
      last_purged_log_id,
      last_log_id,
    })
  }

  async fn get_log_reader(&mut self) -> Self::LogReader {
    self.clone()
  }

  async fn save_vote(&mut self, vote: &VoteOf<C>) -> Result<(), io::Error> {
    self.put_meta::<meta::Vote>(vote)?;
    self.persist_sync().await
  }

  async fn save_committed(&mut self, committed: Option<LogIdOf<C>>) -> Result<(), io::Error> {
    match committed {
      Some(log_id) => self.put_meta::<meta::Committed>(&log_id)?,
      None => self.remove_meta::<meta::Committed>()?,
    }

    self.persist_sync().await
  }

  async fn read_committed(&mut self) -> Result<Option<LogIdOf<C>>, io::Error> {
    self.get_meta::<meta::Committed>()
  }

  async fn append<I>(&mut self, entries: I, callback: IOFlushed<C>) -> Result<(), io::Error>
  where
    I: IntoIterator<Item = EntryOf<C>> + Send,
  {
    let mut batch = self.db.batch();
    for entry in entries {
      let id = id_to_bin(entry.index());
      let value =
        sonic_rs::to_vec(&entry).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
      batch.insert(&self.logs, id, value);
    }

    batch.commit().map_err(read_logs_err)?;

    // Make sure the logs are persisted to disk before invoking the callback.
    //
    // The batch commit happens in this function so the appended entries are
    // readable before returning, while the durable sync can complete later.
    let db = self.db.clone();
    std::thread::spawn(move || {
      let res = db
        .persist(PersistMode::SyncAll)
        .map_err(|e| io::Error::other(e.to_string()));
      callback.io_completed(res);
    });

    Ok(())
  }

  async fn truncate_after(&mut self, last_log_id: Option<LogIdOf<C>>) -> Result<(), io::Error> {
    tracing::debug!("truncate_after: ({:?}, +oo)", last_log_id);

    let Some(start_index) = last_log_id
      .map(|log_id| log_id.index().checked_add(1))
      .unwrap_or(Some(0))
    else {
      return Ok(());
    };

    self.remove_logs_from(start_index)?;
    self.persist_sync().await
  }

  async fn purge(&mut self, log_id: LogIdOf<C>) -> Result<(), io::Error> {
    tracing::debug!("delete_log: [0, {:?}]", log_id);

    self.purge_logs_through(&log_id)?;
    self.persist_sync().await
  }
}

/// Metadata of a raft-store.
///
/// In raft, except logs and state machine, the store also has to store several piece of metadata.
/// This sub mod defines the key-value pairs of these metadata.
mod meta {
  use openraft::{
    RaftTypeConfig,
    alias::{LogIdOf, VoteOf},
  };

  /// Defines metadata key and value
  pub(crate) trait StoreMeta<C>
  where
    C: RaftTypeConfig,
  {
    /// The key used to store in fjall.
    const KEY: &'static str;

    /// The type of the value to store.
    type Value: serde::Serialize + serde::de::DeserializeOwned;
  }

  pub(crate) struct LastPurged {}
  pub(crate) struct Committed {}
  pub(crate) struct Vote {}

  impl<C> StoreMeta<C> for LastPurged
  where
    C: RaftTypeConfig,
  {
    const KEY: &'static str = "last_purged_log_id";
    type Value = LogIdOf<C>;
  }
  impl<C> StoreMeta<C> for Vote
  where
    C: RaftTypeConfig,
  {
    const KEY: &'static str = "vote";
    type Value = VoteOf<C>;
  }

  impl<C> StoreMeta<C> for Committed
  where
    C: RaftTypeConfig,
  {
    const KEY: &'static str = "committed";
    type Value = LogIdOf<C>;
  }
}

use meta::StoreMeta;

/// Converts an id to a byte vector for storing in the database.
/// Big-endian encoding preserves numeric ordering in lexicographic key scans.
fn id_to_bin(id: u64) -> Vec<u8> {
  let mut buf = Vec::with_capacity(8);
  buf
    .write_u64::<BigEndian>(id)
    .expect("writing u64 into Vec cannot fail");
  buf
}

fn bin_to_id(buf: &[u8]) -> u64 {
  (&buf[0 .. 8])
    .read_u64::<BigEndian>()
    .expect("log keys are always encoded as 8-byte u64")
}

fn read_logs_err(e: impl std::error::Error + 'static) -> io::Error {
  io::Error::other(e.to_string())
}

#[cfg(test)]
mod tests {
  use fjall::Database;
  use openraft::{RaftTypeConfig, alias::LogIdOf, storage::RaftLogStorage, vote::RaftLeaderIdExt};

  use super::*;
  use crate::rocksstore_crud::{RocksNodeId, TypeConfig};

  fn log_id(index: u64) -> LogIdOf<TypeConfig> {
    LogIdOf::<TypeConfig>::new(
      <TypeConfig as RaftTypeConfig>::LeaderId::new_committed(1, RocksNodeId::new("node-a")),
      index,
    )
  }

  #[tokio::test]
  async fn fjall_log_store_reopens_committed_log_id() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let committed = log_id(7);

    {
      let db = Database::builder(temp.path())
        .open()
        .expect("open fjall db");
      let mut store = FjallLogStore::<TypeConfig>::new(db).expect("create log store");
      store
        .save_committed(Some(committed.clone()))
        .await
        .expect("save committed");
    }

    {
      let db = Database::builder(temp.path())
        .open()
        .expect("reopen fjall db");
      let mut store = FjallLogStore::<TypeConfig>::new(db).expect("recreate log store");
      assert_eq!(
        Some(committed.clone()),
        store.read_committed().await.expect("read committed")
      );
      store.save_committed(None).await.expect("clear committed");
    }

    {
      let db = Database::builder(temp.path())
        .open()
        .expect("reopen fjall db again");
      let mut store = FjallLogStore::<TypeConfig>::new(db).expect("recreate log store again");
      assert_eq!(
        None,
        store
          .read_committed()
          .await
          .expect("read cleared committed")
      );
    }
  }
}

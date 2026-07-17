use std::{
  cmp, fmt,
  fmt::Debug,
  io,
  marker::PhantomData,
  ops::{Bound, RangeBounds},
  sync::Arc,
};

use openraft::{
  LogState, OptionalSend, RaftLogReader, RaftTypeConfig,
  alias::{EntryOf, LogIdOf, VoteOf},
  entry::RaftEntry,
  storage::{IOFlushed, RaftLogStorage},
};
use rayon::prelude::*;
use rocksdb::{ColumnFamily, DB, Direction, WriteOptions};

const META_CF: &str = "meta";
const LOGS_CF: &str = "logs";
const MAX_LOG_READ_PREALLOC: usize = 1024;
/// Below this many entries a log read deserializes inline: the per-entry
/// JSON work would not amortize the rayon pool dispatch. Reached mostly by
/// replication catch-up and the startup integrity scan, where batches are
/// large and the parallel decode pays off.
const PAR_DESERIALIZE_MIN: usize = 64;

#[derive(Clone)]
pub struct RocksLogStore<C>
where
  C: RaftTypeConfig,
{
  db: Arc<DB>,
  _p: PhantomData<C>,
}

impl<C> Debug for RocksLogStore<C>
where
  C: RaftTypeConfig,
{
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("RocksLogStore").finish_non_exhaustive()
  }
}

impl<C> RocksLogStore<C>
where
  C: RaftTypeConfig,
{
  pub fn new(db: Arc<DB>) -> Result<Self, io::Error> {
    db.cf_handle(META_CF)
      .ok_or_else(|| io::Error::other(format!("column family `{META_CF}` not found")))?;
    db.cf_handle(LOGS_CF)
      .ok_or_else(|| io::Error::other(format!("column family `{LOGS_CF}` not found")))?;

    Ok(Self {
      db,
      _p: PhantomData,
    })
  }

  fn cf_meta(&self) -> &ColumnFamily {
    self
      .db
      .cf_handle(META_CF)
      .expect("column family `meta` not found")
  }

  fn cf_logs(&self) -> &ColumnFamily {
    self
      .db
      .cf_handle(LOGS_CF)
      .expect("column family `logs` not found")
  }

  /// Get a store metadata.
  ///
  /// It returns `None` if the store does not have such a metadata stored.
  fn get_meta<M: StoreMeta<C>>(&self) -> Result<Option<M::Value>, io::Error> {
    let Some(bytes) = self
      .db
      .get_cf(self.cf_meta(), M::KEY)
      .map_err(read_logs_err)?
    else {
      return Ok(None);
    };

    let t =
      sonic_rs::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(Some(t))
  }

  /// Save a store metadata.
  fn put_meta<M: StoreMeta<C>>(&self, value: &M::Value) -> Result<(), io::Error> {
    let value =
      sonic_rs::to_vec(value).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    self
      .db
      .put_cf_opt(self.cf_meta(), M::KEY, value, &durable_write_options())
      .map_err(read_logs_err)
  }

  fn remove_meta<M: StoreMeta<C>>(&self) -> Result<(), io::Error> {
    self
      .db
      .delete_cf_opt(self.cf_meta(), M::KEY, &durable_write_options())
      .map_err(read_logs_err)
  }

  fn remove_logs_from(&self, start_index: u64) -> Result<(), io::Error> {
    let mut batch = rocksdb::WriteBatch::default();
    let deleted = self.append_log_deletes(&mut batch, start_index ..)?;
    if deleted == 0 {
      return Ok(());
    }
    write_batch_sync(&self.db, batch)
  }

  fn purge_logs_through(&self, log_id: &LogIdOf<C>) -> Result<(), io::Error> {
    let mut batch = rocksdb::WriteBatch::default();
    let value =
      sonic_rs::to_vec(log_id).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    batch.put_cf(
      self.cf_meta(),
      <meta::LastPurged as StoreMeta<C>>::KEY,
      value,
    );

    self.append_log_deletes(&mut batch, 0 ..= log_id.index())?;

    write_batch_sync(&self.db, batch)
  }

  fn append_log_deletes<R>(
    &self,
    batch: &mut rocksdb::WriteBatch,
    range: R,
  ) -> Result<usize, io::Error>
  where
    R: RangeBounds<u64>,
  {
    let Some(start_index) = range_start(&range) else {
      return Ok(0);
    };

    let start = id_to_bin(start_index);
    let iter = self.db.iterator_cf(
      self.cf_logs(),
      rocksdb::IteratorMode::From(&start, Direction::Forward),
    );
    let mut deleted = 0;

    for item in iter {
      let (key, _) = item.map_err(read_logs_err)?;
      let id = bin_to_id(key.as_ref())?;
      if !range.contains(&id) {
        break;
      }

      batch.delete_cf(self.cf_logs(), key.as_ref());
      deleted += 1;
    }

    Ok(deleted)
  }
}

impl<C> RaftLogReader<C> for RocksLogStore<C>
where
  C: RaftTypeConfig,
{
  async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
    &mut self,
    range: RB,
  ) -> Result<Vec<C::Entry>, io::Error> {
    let Some(start_index) = range_start(&range) else {
      return Ok(Vec::new());
    };

    let start = id_to_bin(start_index);
    let mut raw = Vec::with_capacity(range_len_hint(&range));
    let iter = self.db.iterator_cf(
      self.cf_logs(),
      rocksdb::IteratorMode::From(&start, Direction::Forward),
    );

    for item in iter {
      let (id, val) = item.map_err(read_logs_err)?;

      let id = bin_to_id(id.as_ref())?;
      if !range.contains(&id) {
        break;
      }

      raw.push((id, val));
    }

    let decode = |(id, val): &(u64, Box<[u8]>)| -> Result<EntryOf<C>, io::Error> {
      let entry: EntryOf<C> = sonic_rs::from_slice(val.as_ref()).map_err(read_logs_err)?;
      if *id != entry.index() {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          format!(
            "log key index {id} does not match entry index {}",
            entry.index()
          ),
        ));
      }
      Ok(entry)
    };

    if raw.len() < PAR_DESERIALIZE_MIN {
      raw.iter().map(decode).collect()
    } else {
      raw.par_iter().map(decode).collect()
    }
  }

  async fn read_vote(&mut self) -> Result<Option<VoteOf<C>>, io::Error> {
    self.get_meta::<meta::Vote>()
  }
}

impl<C> RaftLogStorage<C> for RocksLogStore<C>
where
  C: RaftTypeConfig,
{
  type LogReader = Self;

  async fn get_log_state(&mut self) -> Result<LogState<C>, io::Error> {
    let last = self
      .db
      .iterator_cf(self.cf_logs(), rocksdb::IteratorMode::End)
      .next();

    let last_log_id = match last {
      None => None,
      Some(item) => {
        let (log_index, entry_bytes) = item.map_err(read_logs_err)?;
        let ent =
          sonic_rs::from_slice::<EntryOf<C>>(entry_bytes.as_ref()).map_err(read_logs_err)?;
        let key_index = bin_to_id(log_index.as_ref())?;
        if key_index != ent.index() {
          return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
              "last log key index {key_index} does not match entry index {}",
              ent.index()
            ),
          ));
        }
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
    self.put_meta::<meta::Vote>(vote)
  }

  async fn save_committed(&mut self, committed: Option<LogIdOf<C>>) -> Result<(), io::Error> {
    match committed {
      Some(log_id) => self.put_meta::<meta::Committed>(&log_id),
      None => self.remove_meta::<meta::Committed>(),
    }
  }

  async fn read_committed(&mut self) -> Result<Option<LogIdOf<C>>, io::Error> {
    self.get_meta::<meta::Committed>()
  }

  async fn append<I>(&mut self, entries: I, callback: IOFlushed<C>) -> Result<(), io::Error>
  where
    I: IntoIterator<Item = EntryOf<C>> + Send,
  {
    let mut batch = rocksdb::WriteBatch::default();
    let mut appended = 0;
    for entry in entries {
      let id = id_to_bin(entry.index());
      let value =
        sonic_rs::to_vec(&entry).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
      batch.put_cf(self.cf_logs(), id, value);
      appended += 1;
    }

    if appended == 0 {
      callback.io_completed(Ok(()));
      return Ok(());
    }

    write_batch_sync(&self.db, batch)?;
    callback.io_completed(Ok(()));

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

    self.remove_logs_from(start_index)
  }

  async fn purge(&mut self, log_id: LogIdOf<C>) -> Result<(), io::Error> {
    tracing::debug!("delete_log: [0, {:?}]", log_id);

    self.purge_logs_through(&log_id)
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
    /// The key used to store in RocksDB.
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

/// Converts an id to a fixed-width key for storing in the database.
/// Big-endian encoding preserves numeric ordering in lexicographic key scans.
fn id_to_bin(id: u64) -> [u8; 8] {
  id.to_be_bytes()
}

fn bin_to_id(buf: &[u8]) -> Result<u64, io::Error> {
  let bytes: [u8; 8] = buf.try_into().map_err(|_| {
    io::Error::new(
      io::ErrorKind::InvalidData,
      format!("log key length {} is not 8 bytes", buf.len()),
    )
  })?;
  Ok(u64::from_be_bytes(bytes))
}

fn range_start<R>(range: &R) -> Option<u64>
where
  R: RangeBounds<u64>,
{
  match range.start_bound() {
    Bound::Included(x) => Some(*x),
    Bound::Excluded(x) => x.checked_add(1),
    Bound::Unbounded => Some(0),
  }
}

fn range_len_hint<R>(range: &R) -> usize
where
  R: RangeBounds<u64>,
{
  let Some(start) = range_start(range) else {
    return 0;
  };

  let end_exclusive = match range.end_bound() {
    Bound::Included(x) => x.checked_add(1),
    Bound::Excluded(x) => Some(*x),
    Bound::Unbounded => None,
  };

  end_exclusive
    .and_then(|end| end.checked_sub(start))
    .and_then(|len| usize::try_from(len).ok())
    .map(|len| cmp::min(len, MAX_LOG_READ_PREALLOC))
    .unwrap_or(0)
}

/// Write options for raft storage. The WAL is always enabled; whether each
/// write batch also fsyncs it is governed by the hot-reloadable
/// `rocksdb_sync_writes` runtime config (default `true` = strong single-node
/// durability; `false` trades crash durability for throughput and relies on
/// raft replication — see [`crate::runtime_config::RuntimeConfig`]).
fn durable_write_options() -> WriteOptions {
  let mut opts = WriteOptions::default();
  opts.set_sync(crate::runtime_config::current().rocksdb_sync_writes);
  opts.disable_wal(false);
  opts
}

fn write_batch_sync(db: &DB, batch: rocksdb::WriteBatch) -> Result<(), io::Error> {
  db.write_opt(batch, &durable_write_options())
    .map_err(read_logs_err)
}

fn read_logs_err(e: impl std::error::Error + 'static) -> io::Error {
  io::Error::other(e.to_string())
}

#[cfg(test)]
mod tests {
  use openraft::{
    EntryPayload, RaftLogReader, RaftTypeConfig,
    alias::LogIdOf,
    entry::RaftEntry,
    storage::{IOFlushed, RaftLogStorage},
    vote::RaftLeaderIdExt,
  };
  use rocksdb::{ColumnFamilyDescriptor, Options};

  use super::*;
  use crate::rocksstore_crud::{RocksNodeId, TypeConfig};

  fn log_id(index: u64) -> LogIdOf<TypeConfig> {
    LogIdOf::<TypeConfig>::new(
      <TypeConfig as RaftTypeConfig>::LeaderId::new_committed(1, RocksNodeId::new("node-a")),
      index,
    )
  }

  fn blank_entry(index: u64) -> <TypeConfig as RaftTypeConfig>::Entry {
    <TypeConfig as RaftTypeConfig>::Entry::new(log_id(index), EntryPayload::Blank)
  }

  #[tokio::test]
  async fn rocks_log_store_reopens_committed_log_id() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let committed = log_id(7);

    {
      let db = open_test_db(temp.path());
      let mut store = RocksLogStore::<TypeConfig>::new(db).expect("create log store");
      store
        .save_committed(Some(committed.clone()))
        .await
        .expect("save committed");
    }

    {
      let db = open_test_db(temp.path());
      let mut store = RocksLogStore::<TypeConfig>::new(db).expect("recreate log store");
      assert_eq!(
        Some(committed.clone()),
        store.read_committed().await.expect("read committed")
      );
      store.save_committed(None).await.expect("clear committed");
    }

    {
      let db = open_test_db(temp.path());
      let mut store = RocksLogStore::<TypeConfig>::new(db).expect("recreate log store again");
      assert_eq!(
        None,
        store
          .read_committed()
          .await
          .expect("read cleared committed")
      );
    }
  }

  #[tokio::test]
  async fn rocks_log_store_truncates_and_purges_logs() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let db = open_test_db(temp.path());
    let mut store = RocksLogStore::<TypeConfig>::new(db).expect("create log store");

    store
      .append(
        vec![blank_entry(1), blank_entry(2), blank_entry(3)],
        IOFlushed::noop(),
      )
      .await
      .expect("append logs");

    store
      .truncate_after(Some(log_id(2)))
      .await
      .expect("truncate logs");
    let entries = store
      .try_get_log_entries(0 .. 10)
      .await
      .expect("read truncated logs");
    assert_eq!(
      vec![1, 2],
      entries
        .iter()
        .map(openraft::entry::RaftEntry::index)
        .collect::<Vec<_>>()
    );

    store.purge(log_id(1)).await.expect("purge logs");
    let entries = store
      .try_get_log_entries(0 .. 10)
      .await
      .expect("read purged logs");
    assert_eq!(
      vec![2],
      entries
        .iter()
        .map(openraft::entry::RaftEntry::index)
        .collect::<Vec<_>>()
    );
  }

  fn open_test_db(path: &std::path::Path) -> Arc<DB> {
    let mut opts = Options::default();
    opts.create_missing_column_families(true);
    opts.create_if_missing(true);
    let meta = ColumnFamilyDescriptor::new(META_CF, Options::default());
    let logs = ColumnFamilyDescriptor::new(LOGS_CF, Options::default());
    Arc::new(DB::open_cf_descriptors(&opts, path, vec![meta, logs]).expect("open rocksdb"))
  }
}

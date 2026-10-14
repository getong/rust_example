//! Fixed-membership Raft storage. No log compaction: indexes always start at 1.
use std::{collections::BTreeMap, path::Path, sync::Arc};

use anyhow::{Context, Result, ensure};
use prost::Message as _;
use raft::{GetEntriesContext, RaftState, Storage, StorageError};
use raft_proto::eraftpb::{ConfState, Entry, HardState, Snapshot};
use rocksdb::{DB, Direction, IteratorMode, Options, WriteBatch, WriteOptions};

const OWNER: &[u8] = b"meta/owner-v1";
const CONF: &[u8] = b"meta/conf";
const HARD: &[u8] = b"meta/hard";
const LAST: &[u8] = b"meta/last";
const APPLIED: &[u8] = b"meta/applied";
const STATE: &[u8] = b"state/";

#[derive(Clone)]
pub struct RocksStorage {
  db: Arc<DB>,
}

fn log_key(index: u64) -> [u8; 12] {
  let mut key = *b"log/00000000";
  key[4 ..].copy_from_slice(&index.to_be_bytes());
  key
}

fn storage_error(error: anyhow::Error) -> raft::Error {
  StorageError::Other(Box::new(std::io::Error::other(error.to_string()))).into()
}

impl RocksStorage {
  pub fn open(path: &Path, id: u64, mut voters: Vec<u64>, identity: &str) -> Result<Self> {
    voters.sort_unstable();
    ensure!(
      id != 0
        && voters.contains(&id)
        && !voters.contains(&0)
        && voters.windows(2).all(|pair| pair[0] != pair[1]),
      "invalid voters"
    );
    let mut options = Options::default();
    options.create_if_missing(true);
    let store = Self {
      db: Arc::new(
        DB::open(&options, path).with_context(|| format!("open RocksDB {}", path.display()))?,
      ),
    };
    let owner = serde_json::to_vec(&(id, &voters, identity))?;
    match store.db.get(OWNER)? {
      Some(saved) => ensure!(
        saved == owner,
        "database belongs to a different node, identity or voter set"
      ),
      None => {
        ensure!(
          store.db.iterator(IteratorMode::Start).next().is_none(),
          "database metadata missing"
        );
        let conf = ConfState::from((voters, vec![]));
        let mut batch = WriteBatch::default();
        batch.put(OWNER, owner);
        batch.put(CONF, conf.encode_to_vec());
        batch.put(HARD, HardState::default().encode_to_vec());
        batch.put(LAST, 0_u64.to_be_bytes());
        batch.put(APPLIED, 0_u64.to_be_bytes());
        store.write(batch)?;
      }
    }
    let hard = store.hard_state()?;
    ensure!(
      store.applied()? <= hard.commit && hard.commit <= store.number(LAST)?,
      "invalid applied/commit/last indexes in RocksDB"
    );
    Ok(store)
  }

  fn required(&self, key: &[u8]) -> Result<Vec<u8>> {
    self
      .db
      .get(key)?
      .context("required RocksDB record is missing")
  }

  fn number(&self, key: &[u8]) -> Result<u64> {
    let bytes: [u8; 8] = self
      .required(key)?
      .try_into()
      .map_err(|_| anyhow::anyhow!("invalid index encoding"))?;
    Ok(u64::from_be_bytes(bytes))
  }

  fn write(&self, batch: WriteBatch) -> Result<()> {
    let mut options = WriteOptions::default();
    options.set_sync(true);
    options.disable_wal(false);
    self
      .db
      .write_opt(batch, &options)
      .context("sync RocksDB write batch")
  }

  pub fn hard_state(&self) -> Result<HardState> {
    Ok(HardState::decode(self.required(HARD)?.as_slice())?)
  }

  pub fn applied(&self) -> Result<u64> {
    self.number(APPLIED)
  }

  pub fn values(&self) -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    for item in self
      .db
      .iterator(IteratorMode::From(STATE, Direction::Forward))
    {
      let (key, value) = item?;
      if !key.starts_with(STATE) {
        break;
      }
      values.insert(
        String::from_utf8(key[STATE.len() ..].to_vec())?,
        String::from_utf8(value.to_vec())?,
      );
    }
    Ok(values)
  }

  // Append and HardState share one synced WAL batch. Conflicting uncommitted
  // suffixes are deleted atomically, including entries beyond the new last index.
  pub fn persist(&self, entries: &[Entry], hard: Option<&HardState>) -> Result<()> {
    if entries.is_empty() && hard.is_none() {
      return Ok(());
    }
    let previous = self.hard_state()?;
    let mut last = self.number(LAST)?;
    let mut batch = WriteBatch::default();
    if let (Some(first), Some(end)) = (entries.first(), entries.last()) {
      ensure!(
        first.index > 0 && first.index <= last.saturating_add(1),
        "log gap"
      );
      ensure!(end.index < u64::MAX, "log index overflow");
      ensure!(
        entries
          .windows(2)
          .all(|pair| pair[1].index == pair[0].index + 1),
        "non-contiguous append"
      );
      ensure!(
        end.index >= previous.commit,
        "cannot truncate committed log"
      );
      for entry in entries
        .iter()
        .take_while(|entry| entry.index <= previous.commit)
      {
        ensure!(
          self.required(&log_key(entry.index))? == entry.encode_to_vec(),
          "cannot overwrite committed log"
        );
      }
      if first.index <= last {
        batch.delete_range(log_key(first.index), log_key(last + 1));
      }
      for entry in entries {
        batch.put(log_key(entry.index), entry.encode_to_vec());
      }
      last = end.index;
      batch.put(LAST, last.to_be_bytes());
    }
    if let Some(hard) = hard {
      ensure!(
        hard.term >= previous.term && hard.commit >= previous.commit && hard.commit <= last,
        "invalid HardState transition"
      );
      ensure!(
        hard.term != previous.term || previous.vote == 0 || hard.vote == previous.vote,
        "cannot change vote within the same term"
      );
      batch.put(HARD, hard.encode_to_vec());
    }
    self.write(batch)
  }

  pub fn set_commit(&self, commit: u64) -> Result<()> {
    let mut hard = self.hard_state()?;
    hard.commit = commit;
    self.persist(&[], Some(&hard))
  }

  // Application data and applied index must survive or disappear together.
  pub fn apply(&self, applied: u64, updates: &[(String, String)]) -> Result<()> {
    ensure!(
      applied > self.applied()? && applied <= self.hard_state()?.commit,
      "invalid applied index"
    );
    let mut batch = WriteBatch::default();
    for (key, value) in updates {
      let mut db_key = STATE.to_vec();
      db_key.extend_from_slice(key.as_bytes());
      batch.put(db_key, value.as_bytes());
    }
    batch.put(APPLIED, applied.to_be_bytes());
    self.write(batch)
  }
}

impl Storage for RocksStorage {
  fn initial_state(&self) -> raft::Result<RaftState> {
    (|| {
      Ok(RaftState::new(
        self.hard_state()?,
        ConfState::decode(self.required(CONF)?.as_slice())?,
      ))
    })()
    .map_err(storage_error)
  }

  fn entries(
    &self,
    low: u64,
    high: u64,
    max_size: impl Into<Option<u64>>,
    _: GetEntriesContext,
  ) -> raft::Result<Vec<Entry>> {
    if low < 1 {
      return Err(StorageError::Compacted.into());
    }
    if low > high || high > self.last_index()?.saturating_add(1) {
      return Err(StorageError::Unavailable.into());
    }
    let limit = max_size.into().unwrap_or(u64::MAX);
    (|| {
      let mut entries = Vec::new();
      let mut size = 0_u64;
      for index in low .. high {
        let entry = Entry::decode(self.required(&log_key(index))?.as_slice())?;
        ensure!(entry.index == index, "corrupt log index");
        let next_size = size.saturating_add(entry.encoded_len() as u64);
        if !entries.is_empty() && next_size > limit {
          break;
        }
        size = next_size;
        entries.push(entry);
      }
      Ok(entries)
    })()
    .map_err(storage_error)
  }

  fn term(&self, index: u64) -> raft::Result<u64> {
    if index == 0 {
      return Ok(0);
    }
    if index > self.last_index()? {
      return Err(StorageError::Unavailable.into());
    }
    (|| Ok(Entry::decode(self.required(&log_key(index))?.as_slice())?.term))()
      .map_err(storage_error)
  }

  fn first_index(&self) -> raft::Result<u64> {
    Ok(1)
  }
  fn last_index(&self) -> raft::Result<u64> {
    self.number(LAST).map_err(storage_error)
  }
  fn snapshot(&self, _: u64, _: u64) -> raft::Result<Snapshot> {
    Err(StorageError::SnapshotTemporarilyUnavailable.into())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn entry(index: u64, term: u64) -> Entry {
    Entry {
      index,
      term,
      ..Default::default()
    }
  }

  #[test]
  fn suffix_replacement_and_state_survive_reopen() -> Result<()> {
    let dir = tempfile::tempdir()?;
    {
      let db = RocksStorage::open(dir.path(), 1, vec![1], "peer-1")?;
      db.persist(
        &[entry(1, 1), entry(2, 1), entry(3, 1)],
        Some(&HardState {
          term: 1,
          vote: 1,
          commit: 1,
        }),
      )?;
      db.apply(1, &[("key".into(), "value".into())])?;
      db.persist(
        &[entry(2, 2)],
        Some(&HardState {
          term: 2,
          vote: 1,
          commit: 2,
        }),
      )?;
      assert!(db.persist(&[entry(1, 3)], None).is_err());
      assert!(db.persist(&[entry(4, 2)], None).is_err());
      assert_eq!(
        db.entries(1, 3, Some(0), GetEntriesContext::empty(false))?
          .len(),
        1
      );
      assert!(
        db.entries(0, 1, None, GetEntriesContext::empty(false))
          .is_err()
      );
    }
    let db = RocksStorage::open(dir.path(), 1, vec![1], "peer-1")?;
    assert_eq!(db.last_index()?, 2);
    assert_eq!(db.term(2)?, 2);
    assert!(db.term(3).is_err());
    assert!(db.db.get(log_key(3))?.is_none());
    assert_eq!(db.hard_state()?.commit, 2);
    assert_eq!(db.hard_state()?.vote, 1);
    assert_eq!(db.applied()?, 1);
    assert_eq!(db.values()?["key"], "value");
    drop(db);
    assert!(RocksStorage::open(dir.path(), 1, vec![1], "different-peer").is_err());
    Ok(())
  }
}

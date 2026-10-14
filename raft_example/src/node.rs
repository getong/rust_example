use std::{collections::BTreeMap, path::Path};

use anyhow::{Result, ensure};
use raft::{Config, RawNode, StateRole};
use raft_proto::eraftpb::{Entry, EntryType, Message};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::storage::RocksStorage;

#[derive(Serialize, Deserialize)]
pub struct Set {
  pub key: String,
  pub value: String,
}

pub struct Node {
  pub raft: RawNode<RocksStorage>,
  pub values: BTreeMap<String, String>,
  pub applied: u64,
  pub accept_writes: bool,
}

impl Node {
  pub fn open(id: u64, voters: Vec<u64>, path: &Path, identity: &str) -> Result<Self> {
    let storage = RocksStorage::open(path, id, voters, identity)?;
    Self::from_storage(id, storage)
  }

  fn from_storage(id: u64, storage: RocksStorage) -> Result<Self> {
    let applied = storage.applied()?;
    let values = storage.values()?;
    let config = Config {
      id,
      applied,
      election_tick: 15,
      heartbeat_tick: 3,
      check_quorum: true,
      pre_vote: true,
      max_size_per_msg: 64 * 1024,
      max_inflight_msgs: 16,
      max_uncommitted_size: 1024 * 1024,
      ..Default::default()
    };
    let logger = slog::Logger::root(slog::Discard, slog::o!());
    Ok(Self {
      raft: RawNode::new(&config, storage, &logger)?,
      values,
      applied,
      accept_writes: false,
    })
  }

  // 重启前完成 Ready 持久化；只重建易失状态，保留 term/vote/log/applied。
  pub fn restart(&mut self) -> Result<()> {
    self.ready()?;
    *self = Self::from_storage(self.raft.raft.id, self.raft.store().clone())?;
    self.ready()?;
    Ok(())
  }

  pub fn propose(&mut self, key: String, value: String) -> Result<()> {
    ensure!(
      self.raft.raft.state == StateRole::Leader,
      "not leader; current leader ID is {}",
      self.raft.raft.leader_id
    );
    ensure!(
      self.accept_writes,
      "network has no healthy majority; writes are suspended"
    );
    ensure!(!key.is_empty(), "key cannot be empty");
    let command = serde_json::to_vec(&Set { key, value })?;
    ensure!(command.len() <= 4096, "command exceeds 4096 bytes");
    let index = self.raft.raft.raft_log.last_index() + 1;
    self.raft.propose(vec![], command)?;
    // 仅表示已进入本地日志，不表示已提交；commit 事件在 apply 时输出。
    println!(
      "{}",
      json!({"event":"proposed", "id":self.raft.raft.id,
            "term":self.raft.raft.term, "index":index})
    );
    Ok(())
  }

  fn apply(&mut self, entries: Vec<Entry>) -> Result<()> {
    if entries.is_empty() {
      return Ok(());
    }
    let mut updates = Vec::new();
    let mut commands = Vec::new();
    let mut applied = self.applied;
    for entry in entries {
      ensure!(entry.index == applied + 1, "non-contiguous applied log");
      ensure!(
        entry.get_entry_type() == EntryType::EntryNormal,
        "dynamic membership is not supported"
      );
      if !entry.data.is_empty() {
        let command: Set = serde_json::from_slice(&entry.data)?;
        updates.push((command.key.clone(), command.value.clone()));
        commands.push((entry.term, entry.index, command));
      }
      applied = entry.index;
    }
    // 先原子持久化状态机和 applied index，成功后才能更新缓存/通知客户端。
    self.raft.store().apply(applied, &updates)?;
    self.values.extend(updates);
    self.applied = applied;
    for (term, index, command) in commands {
      println!(
        "{}",
        json!({"event":"committed", "id":self.raft.raft.id,
        "term":term, "index":index, "key":command.key, "value":command.value})
      );
    }
    Ok(())
  }

  pub fn ready(&mut self) -> Result<Vec<Message>> {
    let mut messages = Vec::new();
    while self.raft.has_ready() {
      let mut ready = self.raft.ready();
      let storage = self.raft.store().clone();
      ensure!(
        ready.snapshot().is_empty(),
        "snapshot restore is not supported"
      );
      messages.extend(ready.take_messages());
      storage.persist(ready.entries(), ready.hs())?;
      // RocksDB WAL 同步完成后，才允许发出复制/投票响应。
      messages.extend(ready.take_persisted_messages());
      self.apply(ready.take_committed_entries())?;
      let mut light = self.raft.advance(ready);
      if let Some(commit) = light.commit_index() {
        storage.set_commit(commit)?;
      }
      messages.extend(light.take_messages());
      self.apply(light.take_committed_entries())?;
      self.raft.advance_apply();
    }
    Ok(messages)
  }

  pub fn status(&self) {
    println!(
      "{}",
      json!({"event":"status", "id":self.raft.raft.id,
            "role":format!("{:?}", self.raft.raft.state),
            "leader":self.raft.raft.leader_id, "term":self.raft.raft.term,
            "commit":self.raft.raft.raft_log.committed, "applied":self.applied,
            "values":self.values, "accept_writes":self.accept_writes})
    );
  }
}

#[cfg(test)]
mod tests {
  use raft_proto::eraftpb::HardState;

  use super::*;

  #[test]
  fn replays_committed_but_unapplied_entries_after_reopen() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let command = serde_json::to_vec(&Set {
      key: "key".into(),
      value: "persisted".into(),
    })?;
    {
      let store = RocksStorage::open(dir.path(), 1, vec![1], "peer")?;
      // 模拟日志和 commit 已持久化、状态机尚未应用时进程退出。
      store.persist(
        &[Entry {
          index: 1,
          term: 1,
          data: command,
          ..Default::default()
        }],
        Some(&HardState {
          term: 1,
          vote: 1,
          commit: 1,
        }),
      )?;
    }
    {
      let mut node = Node::open(1, vec![1], dir.path(), "peer")?;
      assert_eq!(node.applied, 0);
      node.ready()?;
      assert_eq!(node.values["key"], "persisted");
      assert_eq!(node.applied, 1);
    }
    let mut node = Node::open(1, vec![1], dir.path(), "peer")?;
    assert_eq!(node.applied, 1);
    assert_eq!(node.values["key"], "persisted");
    node.ready()?;
    assert_eq!(node.applied, 1);
    Ok(())
  }
}

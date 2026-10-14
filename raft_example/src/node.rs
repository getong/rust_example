use std::collections::BTreeMap;

use anyhow::{Result, ensure};
use raft::{Config, RawNode, StateRole, storage::MemStorage};
use raft_proto::eraftpb::{Entry, EntryType, Message};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize)]
pub struct Set {
  pub key: String,
  pub value: String,
}

pub struct Node {
  pub raft: RawNode<MemStorage>,
  pub values: BTreeMap<String, String>,
  pub applied: u64,
}

impl Node {
  pub fn new(id: u64, voters: Vec<u64>) -> Result<Self> {
    let config = Config {
      id,
      election_tick: 15,
      heartbeat_tick: 3,
      check_quorum: true,
      pre_vote: true,
      max_size_per_msg: 64 * 1024,
      max_inflight_msgs: 16,
      max_uncommitted_size: 1024 * 1024,
      ..Default::default()
    };
    let storage = MemStorage::new_with_conf_state((voters, vec![]));
    let logger = slog::Logger::root(slog::Discard, slog::o!());
    Ok(Self {
      raft: RawNode::new(&config, storage, &logger)?,
      values: BTreeMap::new(),
      applied: 0,
    })
  }

  pub fn propose(&mut self, key: String, value: String) -> Result<()> {
    ensure!(
      self.raft.raft.state == StateRole::Leader,
      "not leader; current leader ID is {}",
      self.raft.raft.leader_id
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
    for entry in entries {
      ensure!(
        entry.get_entry_type() == EntryType::EntryNormal,
        "dynamic membership is not supported"
      );
      if !entry.data.is_empty() {
        let command: Set = serde_json::from_slice(&entry.data)?;
        self
          .values
          .insert(command.key.clone(), command.value.clone());
        println!(
          "{}",
          json!({"event":"committed", "id":self.raft.raft.id,
                    "term":entry.term, "index":entry.index, "key":command.key, "value":command.value})
        );
      }
      self.applied = entry.index;
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
      storage.wl().append(ready.entries())?;
      if let Some(hard_state) = ready.hs() {
        storage.wl().set_hardstate(hard_state.clone());
      }
      // MemStorage 在此完成保存；生产环境必须等 WAL 真正持久化完成。
      messages.extend(ready.take_persisted_messages());
      self.apply(ready.take_committed_entries())?;
      let mut light = self.raft.advance(ready);
      if let Some(commit) = light.commit_index() {
        storage.wl().mut_hard_state().set_commit(commit);
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
            "values":self.values})
    );
  }
}

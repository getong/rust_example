use std::{
  collections::{BTreeMap, VecDeque},
  error::Error,
};

use raft::{
  StateRole,
  prelude::{Config, Entry, EntryType, Message, RawNode},
  storage::MemStorage,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Node {
  raft: RawNode<MemStorage>,
  values: BTreeMap<String, String>,
  applied: u64,
  online: bool,
}

impl Node {
  fn new(id: u64) -> Result<Self> {
    let config = Config {
      id,
      election_tick: 10,
      heartbeat_tick: 2,
      check_quorum: true,
      pre_vote: true,
      ..Default::default()
    };
    let storage = MemStorage::new_with_conf_state((vec![1, 2, 3], vec![]));
    let logger = slog::Logger::root(slog::Discard, slog::o!());
    Ok(Self {
      raft: RawNode::new(&config, storage, &logger)?,
      values: BTreeMap::new(),
      applied: 0,
      online: true,
    })
  }

  fn apply(&mut self, entries: Vec<Entry>) -> Result<()> {
    for entry in entries {
      if entry.get_entry_type() != EntryType::EntryNormal {
        return Err("this example only supports fixed membership".into());
      }
      // 新 Leader 的空日志也需要推进 applied index。
      if !entry.data.is_empty() {
        let command = std::str::from_utf8(&entry.data)?;
        let (key, value) = command.split_once('=').ok_or("invalid SET command")?;
        self.values.insert(key.to_owned(), value.to_owned());
      }
      self.applied = entry.index;
    }
    Ok(())
  }

  fn on_ready(&mut self, messages: &mut VecDeque<Message>) -> Result<()> {
    if !self.raft.has_ready() {
      return Ok(());
    }
    let mut ready = self.raft.ready();
    let storage = self.raft.store().clone();
    // 不裁剪日志，因此不会产生快照；状态机快照恢复不在本例范围内。
    if !ready.snapshot().is_empty() {
      return Err("snapshot restore is outside this example".into());
    }
    messages.extend(ready.take_messages());
    storage.wl().append(ready.entries())?;
    if let Some(hard_state) = ready.hs() {
      storage.wl().set_hardstate(hard_state.clone());
    }
    // 这类消息必须等日志和 HardState 保存完成后才能发送。
    messages.extend(ready.take_persisted_messages());
    self.apply(ready.take_committed_entries())?;
    let mut light = self.raft.advance(ready);
    if let Some(commit) = light.commit_index() {
      storage.wl().mut_hard_state().set_commit(commit);
    }
    messages.extend(light.take_messages());
    self.apply(light.take_committed_entries())?;
    self.raft.advance_apply();
    Ok(())
  }
}

struct Cluster {
  nodes: BTreeMap<u64, Node>,
}

impl Cluster {
  fn new() -> Result<Self> {
    let mut nodes = BTreeMap::new();
    for id in 1 ..= 3 {
      nodes.insert(id, Node::new(id)?);
    }
    Ok(Self { nodes })
  }

  // 用内存队列代替网络；离线节点不运行，也不接收消息。
  fn drain(&mut self) -> Result<()> {
    for _ in 0 .. 1000 {
      let mut messages = VecDeque::new();
      let mut progressed = false;
      for node in self.nodes.values_mut().filter(|node| node.online) {
        if node.raft.has_ready() {
          progressed = true;
          node.on_ready(&mut messages)?;
        }
      }
      for message in messages {
        if let Some(node) = self.nodes.get_mut(&message.to).filter(|node| node.online) {
          node.raft.step(message)?;
        }
      }
      if !progressed {
        return Ok(());
      }
    }
    Err("message processing did not settle".into())
  }

  fn tick(&mut self) -> Result<()> {
    for node in self.nodes.values_mut().filter(|node| node.online) {
      node.raft.tick();
    }
    self.drain()
  }

  fn leader(&self) -> Option<u64> {
    self.nodes.iter().find_map(|(&id, node)| {
      (node.online && node.raft.raft.state == StateRole::Leader).then_some(id)
    })
  }

  fn wait_for_leader(&mut self) -> Result<u64> {
    for _ in 0 .. 1000 {
      if let Some(id) = self.leader() {
        return Ok(id);
      }
      self.tick()?;
    }
    Err("leader election timed out".into())
  }

  fn set(&mut self, key: &str, value: &str) -> Result<()> {
    if key.is_empty() || key.contains('=') {
      return Err("key must be nonempty and cannot contain '='".into());
    }
    let leader = self.leader().ok_or("no online leader")?;
    let node = self.nodes.get_mut(&leader).ok_or("leader missing")?;
    let index = node.raft.raft.raft_log.last_index() + 1;
    let term = node.raft.raft.term;
    node
      .raft
      .propose(vec![], format!("{key}={value}").into_bytes())?;
    self.drain()?;
    let node = &self.nodes[&leader];
    // propose 成功只表示接收提案；必须检查提案确实提交并应用。
    if node.applied < index || node.raft.raft.term != term {
      return Err("proposal not confirmed committed (quorum unavailable or leader changed)".into());
    }
    println!("committed: leader={leader}, index={index}, {key}={value}");
    Ok(())
  }

  fn set_online(&mut self, id: u64, online: bool) -> Result<()> {
    self.nodes.get_mut(&id).ok_or("unknown node")?.online = online;
    Ok(())
  }

  fn print_state(&self) {
    for (id, node) in &self.nodes {
      println!(
        "node {id}: online={}, role={:?}, term={}, applied={}, values={:?}",
        node.online, node.raft.raft.state, node.raft.raft.term, node.applied, node.values
      );
    }
  }
}

fn main() -> Result<()> {
  let mut cluster = Cluster::new()?;
  let old_leader = cluster.wait_for_leader()?;
  println!("elected leader: {old_leader}");
  cluster.set("name", "raft")?;
  cluster.print_state();

  println!("\nstop leader {old_leader}");
  cluster.set_online(old_leader, false)?;
  let new_leader = cluster.wait_for_leader()?;
  println!("new leader: {new_leader}");
  cluster.set("name", "raft-after-failover")?;
  cluster.print_state();

  println!("\nresume node {old_leader}");
  cluster.set_online(old_leader, true)?;
  for _ in 0 .. 20 {
    cluster.tick()?;
  }
  cluster.print_state();
  for node in cluster.nodes.values() {
    if node.values.get("name").map(String::as_str) != Some("raft-after-failover") {
      return Err("replicas did not converge".into());
    }
  }
  println!("all three replicas agree");
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn replication_survives_failover_and_catches_up() -> Result<()> {
    let mut cluster = Cluster::new()?;
    let old = cluster.wait_for_leader()?;
    cluster.set("key", "before")?;
    assert!(
      cluster
        .nodes
        .values()
        .all(|node| node.values["key"] == "before")
    );
    cluster.set_online(old, false)?;
    assert_ne!(cluster.wait_for_leader()?, old);
    cluster.set("key", "after=failover")?;
    assert_eq!(cluster.nodes[&old].values["key"], "before");
    cluster.set_online(old, true)?;
    for _ in 0 .. 20 {
      cluster.tick()?;
    }
    assert!(
      cluster
        .nodes
        .values()
        .all(|node| node.values["key"] == "after=failover")
    );
    Ok(())
  }

  #[test]
  fn minority_cannot_apply_a_proposal() -> Result<()> {
    let mut cluster = Cluster::new()?;
    let leader = cluster.wait_for_leader()?;
    for id in 1 ..= 3 {
      if id != leader {
        cluster.set_online(id, false)?;
      }
    }
    assert!(cluster.set("key", "uncommitted").is_err());
    assert!(
      cluster
        .nodes
        .values()
        .all(|node| !node.values.contains_key("key"))
    );
    Ok(())
  }
}

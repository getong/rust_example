//! Owns one Raft instance and one network generation. Storage faults are fatal;
//! transport listener faults trigger bounded, delayed recovery from RocksDB.
use std::{
  collections::BTreeSet,
  time::{Duration, Instant},
};

use anyhow::{Result, bail, ensure};
use libp2p::identity::Keypair;
use serde_json::json;

use crate::{
  config::NodeConfig,
  network::{Event, Network},
  node::Node,
};

pub struct Service {
  pub node: Node,
  pub network: Option<Network>,
  config: NodeConfig,
  key: Keypair,
  blocked: BTreeSet<u64>,
  attempts: u32,
  next_attempt: Instant,
  running_since: Option<Instant>,
  generation: u64,
  last_report: Instant,
}

impl Service {
  pub fn new(node: Node, config: NodeConfig, key: Keypair) -> Self {
    Self {
      node,
      network: None,
      config,
      key,
      blocked: BTreeSet::new(),
      attempts: 0,
      next_attempt: Instant::now(),
      running_since: None,
      generation: 0,
      last_report: Instant::now(),
    }
  }

  fn lifecycle(&self, state: &str, reason: &str) {
    println!(
      "{}",
      json!({"event":"lifecycle", "id":self.config.id, "state":state,
            "generation":self.generation, "attempt":self.attempts, "reason":reason})
    );
  }

  fn recover(&mut self, reason: &str) -> Result<()> {
    self.node.accept_writes = false;
    // Drop the entire Swarm, including pending requests and stale connections.
    self.network = None;
    self.running_since = None;
    self.lifecycle("recovering", reason);
    self.node.restart()?;
    let delay = Duration::from_millis(500 * (1_u64 << self.attempts.min(4)));
    self.next_attempt = Instant::now() + delay;
    Ok(())
  }

  pub fn tick(&mut self) -> Result<()> {
    if self.network.is_none() && Instant::now() >= self.next_attempt {
      ensure!(
        self.attempts < 5,
        "network recovery exhausted after five attempts"
      );
      self.attempts += 1;
      match Network::new(&self.config, self.key.clone()) {
        Ok(mut network) => {
          for &id in &self.blocked {
            network.set_blocked(id, true, &mut self.node)?;
          }
          self.generation += 1;
          self.network = Some(network);
          self.lifecycle("starting", "opening listener");
        }
        Err(error) => self.recover(&error.to_string())?,
      }
    }
    if let Some(network) = &mut self.network {
      network.monitor(&mut self.node);
      network.connect_peers();
      // Isolation does not stop Raft's logical clock: check_quorum must be
      // able to demote a leader. Never remove voters on transport failure.
      self.node.raft.tick();
    }
    if self
      .running_since
      .is_some_and(|since| since.elapsed() >= Duration::from_secs(10))
    {
      self.attempts = 0;
    }
    self.node.accept_writes = self.node.raft.raft.state == raft::StateRole::Leader
      && self.network.as_ref().is_some_and(Network::has_majority);
    if self.last_report.elapsed() >= Duration::from_secs(1) {
      self.status();
      self.last_report = Instant::now();
    }
    Ok(())
  }

  pub fn handle(&mut self, event: Event) -> Result<()> {
    if let Some(network) = &mut self.network {
      if let Err(error) = network.handle(event, &mut self.node) {
        // Storage errors are not repaired by rebuilding a network. Stop
        // instead of continuing from a possibly incomplete durable state.
        if matches!(
          error.downcast_ref::<raft::Error>(),
          Some(raft::Error::Store(_))
        ) {
          return Err(error);
        }
        self.recover(&error.to_string())?;
        return Ok(());
      }
      if network.listening && self.running_since.is_none() {
        self.running_since = Some(Instant::now());
        self.lifecycle("running", "listener ready");
      }
    }
    Ok(())
  }

  pub fn flush(&mut self) -> Result<()> {
    let messages = self.node.ready()?;
    if let Some(network) = &mut self.network {
      network.send(messages, &mut self.node);
    }
    self.node.accept_writes = self.node.raft.raft.state == raft::StateRole::Leader
      && self.network.as_ref().is_some_and(Network::has_majority);
    Ok(())
  }

  pub fn status(&self) {
    self.node.status();
    if let Some(network) = &self.network {
      network.status(self.config.id);
    } else {
      self.lifecycle("recovering", "waiting before next network attempt");
    }
  }

  pub fn command(&mut self, line: &str) -> Result<()> {
    match line {
      "status" | "network" => {
        self.status();
        return Ok(());
      }
      // Process this in the main loop as an operational action, not a
      // client validation error: failures must terminate the service.
      "restart" => bail!("restart must be handled by the supervisor"),
      _ => {}
    }
    if let Some((action, value)) = line.split_once(' ') {
      if action == "disconnect" || action == "reconnect" {
        let id: u64 = value.trim().parse()?;
        ensure!(
          id != self.config.id && self.config.peers.iter().any(|peer| peer.id == id),
          "unknown remote node ID"
        );
        let blocked = action == "disconnect";
        if let Some(network) = &mut self.network {
          network.set_blocked(id, blocked, &mut self.node)?;
        }
        if blocked {
          self.blocked.insert(id);
        } else {
          self.blocked.remove(&id);
        }
        self.node.accept_writes = self.node.raft.raft.state == raft::StateRole::Leader
          && self.network.as_ref().is_some_and(Network::has_majority);
        self.status();
        return Ok(());
      }
      if action == "set"
        && let Some((key, value)) = value.trim_start().split_once(' ')
      {
        return self.node.propose(key.to_owned(), value.to_owned());
      }
    }
    bail!(
      "expected: set <key> <value> | status | network | disconnect <id> | reconnect <id> | \
       restart | quit"
    )
  }

  pub fn network_closed(&mut self) -> Result<()> {
    self.recover("network event stream ended")
  }

  pub fn restart(&mut self) -> Result<()> {
    self.attempts = 0;
    self.recover("operator requested restart")
  }

  pub fn failed(&mut self, reason: &str) {
    self.node.accept_writes = false;
    self.network = None;
    self.lifecycle("failed", reason);
  }

  pub fn shutdown(&mut self) -> Result<()> {
    self.node.accept_writes = false;
    self.node.ready()?;
    self.network = None;
    self.lifecycle("stopped", "shutdown complete");
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use futures::StreamExt;

  use super::*;
  use crate::config;

  fn service() -> Result<(tempfile::TempDir, Service)> {
    let dir = tempfile::tempdir()?;
    config::init(
      dir.path(),
      (1 ..= 3)
        .map(|id| format!("/ip4/127.0.0.1/tcp/{}", 7000 + id))
        .collect(),
    )?;
    let (mut config, key) = NodeConfig::load(&dir.path().join("node1.json"))?;
    config.listen = "/ip4/127.0.0.1/tcp/0".into();
    let node = Node::open(1, vec![1, 2, 3], &dir.path().join("db"), "test")?;
    let mut service = Service::new(node, config, key);
    service.command("disconnect 2")?;
    service.command("disconnect 3")?;
    Ok((dir, service))
  }

  #[tokio::test]
  async fn listener_loss_rebuilds_network_and_preserves_raft_state() -> Result<()> {
    let (_dir, mut service) = service()?;
    service.tick()?;
    // Poll only the ephemeral local listener; peer addresses are not dialed here.
    while !service.network.as_ref().unwrap().listening {
      let event = tokio::time::timeout(
        Duration::from_secs(3),
        service.network.as_mut().unwrap().swarm.select_next_some(),
      )
      .await?;
      service.handle(event)?;
    }
    let term = service.node.raft.raft.term;
    let network = service.network.as_mut().unwrap();
    network.swarm.remove_listener(network.listener);
    while service.network.is_some() {
      let event = tokio::time::timeout(
        Duration::from_secs(3),
        service.network.as_mut().unwrap().swarm.select_next_some(),
      )
      .await?;
      service.handle(event)?;
    }
    assert!(!service.node.accept_writes);
    assert_eq!(service.node.raft.raft.term, term);
    assert_eq!(service.node.raft.raft.state, raft::StateRole::Follower);
    service.next_attempt = Instant::now();
    service.tick()?;
    assert_eq!(service.generation, 2);
    assert!(service.network.is_some());
    Ok(())
  }

  #[tokio::test]
  async fn recovery_budget_stops_repeated_listener_failures() -> Result<()> {
    let (_dir, mut service) = service()?;
    service.attempts = 5;
    assert!(service.tick().is_err());
    assert!(!service.node.accept_writes);
    assert!(service.network.is_none());
    Ok(())
  }
}

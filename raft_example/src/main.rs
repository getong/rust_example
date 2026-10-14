mod config;
mod network;
mod node;

use std::{
  io::{self, BufRead},
  path::Path,
  time::Duration,
};

use anyhow::{Result, bail};
use futures::StreamExt;
use serde_json::json;
use tokio::{
  sync::mpsc,
  time::{self, MissedTickBehavior},
};

use crate::{config::NodeConfig, network::Network, node::Node};

const HELP: &str = "Usage:
  raft_example init [directory] [address1 address2 address3]
  raft_example node <config.json>

Defaults: directory=cluster, addresses=/ip4/127.0.0.1/tcp/7001..7003
Node commands on stdin: set <key> <value>, status, quit
Each node is a separate process. Storage is in memory; do not restart one node
with the same identity in a running cluster after losing its state.";

#[tokio::main]
async fn main() -> Result<()> {
  let args: Vec<_> = std::env::args().skip(1).collect();
  match args.first().map(String::as_str) {
    Some("init") if args.len() <= 2 || args.len() == 5 => {
      let directory = args.get(1).map(String::as_str).unwrap_or("cluster");
      let addresses = if args.len() == 5 {
        args[2 ..].to_vec()
      } else {
        (7001 ..= 7003)
          .map(|port| format!("/ip4/127.0.0.1/tcp/{port}"))
          .collect()
      };
      config::init(Path::new(directory), addresses)
    }
    Some("node") if args.len() == 2 => run(Path::new(&args[1])).await,
    None | Some("--help" | "-h") => {
      println!("{HELP}");
      Ok(())
    }
    _ => bail!("{HELP}"),
  }
}

// 独立线程读取阻塞 stdin，有界通道送进 Tokio；避免 Tokio stdin 的阻塞读取
// 导致 Ctrl-C 后 runtime 等待输入线程而不能退出。该线程不拥有 Raft 状态。
fn commands() -> mpsc::Receiver<String> {
  let (sender, receiver) = mpsc::channel(32);
  std::thread::spawn(move || {
    for line in io::stdin().lock().lines() {
      match line {
        Ok(line) if line.len() <= 8192 => {
          if sender.blocking_send(line).is_err() {
            break;
          }
        }
        Ok(_) => eprintln!("command line exceeds 8192 bytes"),
        Err(error) => {
          eprintln!("stdin: {error}");
          break;
        }
      }
    }
  });
  receiver
}

async fn run(path: &Path) -> Result<()> {
  let (config, key) = NodeConfig::load(path)?;
  let mut node = Node::new(config.id, config.peers.iter().map(|peer| peer.id).collect())?;
  let mut network = Network::new(&config, key)?;
  let mut commands = commands();
  let mut stdin_open = true;
  let mut tick = time::interval(Duration::from_millis(100));
  tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
  let stop = tokio::signal::ctrl_c();
  tokio::pin!(stop);
  let mut last_role = (
    node.raft.raft.state,
    node.raft.raft.leader_id,
    node.raft.raft.term,
  );
  eprintln!(
    "node {} started; commands: set <key> <value> | status | quit",
    config.id
  );
  loop {
    tokio::select! {
        _ = tick.tick() => { node.raft.tick(); }
        event = network.swarm.select_next_some() => { network.handle(event, &mut node)?; }
        line = commands.recv(), if stdin_open => {
            match line {
                Some(line) => {
                    let line = line.trim();
                    if line == "quit" { break; }
                    if let Err(error) = command(line, &mut node) {
                        println!("{}", json!({"event":"error", "id":config.id, "message":error.to_string()}));
                    }
                }
                None => stdin_open = false, // 后台运行时 stdin EOF 不停止节点。
            }
        }
        result = &mut stop => { result?; break; }
    }
    let messages = node.ready()?;
    network.send(messages, &mut node);
    let role = (
      node.raft.raft.state,
      node.raft.raft.leader_id,
      node.raft.raft.term,
    );
    if role != last_role {
      node.status();
      last_role = role;
    }
  }
  Ok(())
}

fn command(line: &str, node: &mut Node) -> Result<()> {
  if line == "status" {
    node.status();
    return Ok(());
  }
  if let Some(rest) = line.strip_prefix("set ")
    && let Some((key, value)) = rest.trim_start().split_once(' ')
  {
    return node.propose(key.to_owned(), value.to_owned());
  }
  bail!("expected: set <key> <value> | status | quit")
}

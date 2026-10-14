mod config;
mod network;
mod node;
mod service;
mod storage;

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

use crate::{config::NodeConfig, node::Node, service::Service};

const HELP: &str = "Usage:
  raft_example init [directory] [address1 address2 address3]
  raft_example node <config.json>

Defaults: directory=cluster, addresses=/ip4/127.0.0.1/tcp/7001..7003
Node commands: set <key> <value>, status, network, disconnect <id>, reconnect <id>, restart, quit
Each node persists Raft and application state in its own RocksDB data_dir.
Restart with the same configuration and database to recover.";

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
  let data_dir = config
    .data_dir
    .clone()
    .unwrap_or_else(|| format!("node{}.rocksdb", config.id).into());
  let data_dir = if data_dir.is_absolute() {
    data_dir
  } else {
    path.parent().unwrap_or(Path::new(".")).join(data_dir)
  };
  let membership: std::collections::BTreeMap<_, _> = config
    .peers
    .iter()
    .map(|peer| (peer.id, &peer.peer_id))
    .collect();
  let identity = serde_json::to_string(&membership)?;
  let node = Node::open(
    config.id,
    config.peers.iter().map(|peer| peer.id).collect(),
    &data_dir,
    &identity,
  )?;
  let mut service = Service::new(node, config, key);
  eprintln!("RocksDB: {}", data_dir.display());
  let mut commands = commands();
  let mut stdin_open = true;
  let mut tick = time::interval(Duration::from_millis(100));
  tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
  let stop = tokio::signal::ctrl_c();
  tokio::pin!(stop);
  loop {
    let outcome: Result<bool> = async {
      tokio::select! {
        _ = tick.tick() => service.tick()?,
        event = async {
          match &mut service.network {
            Some(network) => network.swarm.next().await,
            None => std::future::pending().await,
          }
        } => match event {
          Some(event) => service.handle(event)?,
          None => service.network_closed()?,
        },
        line = commands.recv(), if stdin_open => {
          match line {
            Some(line) => {
              let line = line.trim();
              if line == "quit" { return Ok(false); }
              if line == "restart" {
                service.restart()?;
              } else if let Err(error) = service.command(line) {
                if matches!(error.downcast_ref::<raft::Error>(), Some(raft::Error::Store(_))) {
                  return Err(error);
                }
                println!("{}", json!({"event":"error", "id":service.node.raft.raft.id,
                  "message":error.to_string()}));
              }
            }
            None => stdin_open = false,
          }
        }
        result = &mut stop => { result?; return Ok(false); }
      }
      service.flush()?;
      Ok(true)
    }
    .await;
    match outcome {
      Ok(true) => {}
      Ok(false) => {
        if let Err(error) = service.shutdown() {
          service.failed(&error.to_string());
          return Err(error);
        }
        return Ok(());
      }
      Err(error) => {
        service.failed(&error.to_string());
        return Err(error);
      }
    }
  }
}

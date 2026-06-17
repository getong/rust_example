use std::{path::PathBuf, time::Duration};

use anyhow::{Context, anyhow};
use clap::Parser;
use futures::StreamExt;
use libp2p::{Multiaddr, PeerId, Swarm, identity};
use openraft::ServerState;
use tokio::{select, sync::mpsc};

use crate::{
  domain::{DistributedTask, TaskStage},
  journal::ConsensusTaskJournal,
  network,
  raft_role::OpenRaftRoleTracker,
  state::{AppState, TaskDispatch},
  worker::spawn_apalis_worker,
};

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub(crate) struct Opt {
  /// Local libp2p listen address.
  #[arg(long, default_value = "/ip4/0.0.0.0/udp/0/quic-v1")]
  listen: Multiaddr,

  /// Dial an existing peer. mDNS is also enabled for local discovery.
  #[arg(long)]
  peer: Vec<Multiaddr>,

  /// RocksDB directory for the task understanding journal.
  #[arg(long, default_value = "./data/libp2p-task-journal")]
  db: PathBuf,

  /// Publish one demo task after the node starts.
  #[arg(long, default_value_t = false)]
  publish_demo: bool,

  /// Keep running after the demo task is processed.
  #[arg(long, default_value_t = false)]
  keep_alive: bool,

  /// Local OpenRaft node id. Defaults to the local libp2p PeerId string.
  #[arg(long)]
  openraft_node_id: Option<String>,

  /// Local OpenRaft server state: learner, follower, candidate, leader, or shutdown.
  #[arg(long, value_parser = parse_openraft_state)]
  openraft_state: Option<ServerState>,

  /// Current OpenRaft leader node id, used only for journal detail.
  #[arg(long)]
  openraft_leader: Option<String>,
}

pub(crate) async fn run(opt: Opt) -> anyhow::Result<()> {
  let local_key = identity::Keypair::generate_ed25519();
  let local_peer_id = PeerId::from(local_key.public());
  let topic = network::task_topic();

  let journal = ConsensusTaskJournal::open(&opt.db)?;
  let (incoming_tx, incoming_rx) = mpsc::channel(128);
  let (worker_done_tx, mut worker_done_rx) = mpsc::channel(128);
  let openraft_node_id = opt
    .openraft_node_id
    .clone()
    .unwrap_or_else(|| local_peer_id.to_string());
  let raft_role = OpenRaftRoleTracker::new(openraft_node_id.clone(), None, None);
  raft_role.set_state(opt.openraft_state).await;
  raft_role
    .set_current_leader(opt.openraft_leader.clone())
    .await;

  let state = AppState::new(
    local_peer_id,
    journal.clone(),
    incoming_tx.clone(),
    worker_done_tx,
    raft_role.clone(),
  );

  spawn_apalis_worker(incoming_rx, state.clone());

  let mut swarm = network::build_swarm(local_key, &topic)?;
  swarm
    .listen_on(opt.listen.clone())
    .context("listen on libp2p")?;
  for peer in &opt.peer {
    Swarm::dial(&mut swarm, peer.clone()).with_context(|| format!("dial peer {peer}"))?;
  }

  tokio::spawn(network::report_ready_address(
    local_peer_id,
    opt.listen.clone(),
    network::swarm_external_addresses(&swarm),
    openraft_node_id,
    opt.openraft_state,
    opt.openraft_leader.clone(),
  ));

  let mut published_demo = false;
  let publish_deadline = tokio::time::sleep(Duration::from_millis(500));
  tokio::pin!(publish_deadline);

  loop {
    select! {
      _ = &mut publish_deadline, if opt.publish_demo && !published_demo => {
        published_demo = true;
        let task = DistributedTask::demo(local_peer_id);
        network::publish_task(&mut swarm, &topic, &journal, &task).await?;
        let dispatch = state.enqueue_task_once(
          task,
          TaskStage::SubmittedLocally,
          "task published locally and evaluated against openraft role before apalis enqueue",
        ).await?;
        if !opt.keep_alive && dispatch != TaskDispatch::Enqueued {
          print_journal(&journal).await?;
          return Ok(());
        }
      }
      Some(task) = worker_done_rx.recv() => {
        if !opt.keep_alive && opt.publish_demo && task.submitted_by == local_peer_id.to_string() {
          print_journal(&journal).await?;
          return Ok(());
        }
      }
      event = swarm.select_next_some() => {
        network::handle_swarm_event(event, &mut swarm, &topic, &state).await?;
      }
      _ = tokio::signal::ctrl_c() => {
        print_journal(&journal).await?;
        return Ok(());
      }
    }
  }
}

async fn print_journal(journal: &ConsensusTaskJournal) -> anyhow::Result<()> {
  let rows = journal.list().await?;
  println!("task understanding journal:");
  for row in rows {
    println!(
      "  task={} stage={:?} node={} detail={}",
      row.task_id, row.stage, row.node, row.detail
    );
  }
  Ok(())
}

fn parse_openraft_state(value: &str) -> anyhow::Result<ServerState> {
  match value {
    "learner" => Ok(ServerState::Learner),
    "follower" => Ok(ServerState::Follower),
    "candidate" => Ok(ServerState::Candidate),
    "leader" => Ok(ServerState::Leader),
    "shutdown" => Ok(ServerState::Shutdown),
    _ => Err(anyhow!(
      "invalid openraft state `{value}`; expected learner, follower, candidate, leader, or \
       shutdown"
    )),
  }
}

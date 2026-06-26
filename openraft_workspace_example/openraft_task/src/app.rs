use std::{
  collections::{BTreeMap, BTreeSet},
  path::PathBuf,
  sync::Arc,
  time::Duration,
};

use anyhow::Context;
use apalis::prelude::{Task, TaskId, TaskSink};
use clap::Parser;
use openraft::{BasicNode, Config, async_runtime::WatchReceiver};
use tokio::task::JoinSet;

use crate::{
  Raft,
  apalis_raft::{DemoTask, RaftApalisStorage, RaftTaskId, run_demo_worker},
  network::Router,
  rocksstore_crud::RocksStateMachine,
  store,
};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about)]
pub struct Opt {
  /// Directory used for RocksDB data.
  #[arg(long, default_value = "target/openraft-task-data")]
  pub db: PathBuf,

  /// Number of demo tasks to submit through the Apalis storage.
  #[arg(long, default_value_t = 5)]
  pub tasks: usize,

  /// How long the demo worker should poll before shutting down.
  #[arg(long, default_value_t = 3)]
  pub worker_seconds: u64,
}

pub async fn run(opt: Opt) -> anyhow::Result<()> {
  let cluster = LocalCluster::start(opt.db.clone()).await?;
  cluster.initialize().await?;
  cluster.wait_for_leader(Duration::from_secs(5)).await?;

  let leader_id = cluster.leader_id().await?;
  let leader = cluster
    .node(leader_id)
    .with_context(|| format!("leader node {leader_id} not found"))?;

  let mut storage = RaftApalisStorage::<DemoTask>::new(
    "demo",
    leader.raft.clone(),
    cluster.router.clone(),
    leader.state_machine.clone(),
  );

  for index in 0 .. opt.tasks {
    let task_id = format!("task-{index}");
    storage
      .push_task(
        Task::builder(DemoTask {
          task_id: task_id.clone(),
          payload: format!("payload-{index}"),
        })
        .with_task_id(TaskId::new(RaftTaskId::new(task_id.clone())))
        .build(),
      )
      .await
      .map_err(|err| anyhow::anyhow!("push {task_id}: {err}"))?;
  }

  run_demo_worker(
    format!("worker-node-{leader_id}"),
    storage.clone(),
    Duration::from_secs(opt.worker_seconds),
  )
  .await?;

  let tasks = storage.list_tasks().await?;
  for task in tasks {
    println!(
      "{} status={} attempts={} worker={:?} result_ok={:?} error={:?}",
      task.task_id, task.status, task.attempts, task.lock_by, task.result_ok, task.error
    );
  }

  Ok(())
}

#[derive(Clone)]
struct LocalNode {
  raft: Raft,
  state_machine: RocksStateMachine,
}

struct LocalCluster {
  nodes: BTreeMap<u64, LocalNode>,
  router: Router,
}

impl LocalCluster {
  async fn start(db_dir: PathBuf) -> anyhow::Result<Self> {
    let router = Router::default();
    let config = Arc::new(
      Config {
        heartbeat_interval: 250,
        election_timeout_min: 800,
        election_timeout_max: 1200,
        ..Default::default()
      }
      .validate()
      .context("validate raft config")?,
    );

    let mut nodes = BTreeMap::new();
    for node_id in 1 ..= 3 {
      let node_dir = store::node_db_dir(&db_dir, node_id);
      let (log_store, state_machine) = store::open_store(&node_dir).await?;
      let raft = Raft::new(
        node_id,
        config.clone(),
        router.clone(),
        log_store,
        state_machine.clone(),
      )
      .await
      .context("create raft node")?;
      router.insert(node_id, raft.clone()).await;
      nodes.insert(
        node_id,
        LocalNode {
          raft,
          state_machine,
        },
      );
    }

    Ok(Self { nodes, router })
  }

  async fn initialize(&self) -> anyhow::Result<()> {
    let leader = self.node(1).context("node 1 not found")?;
    let mut members = BTreeMap::new();
    members.insert(1, BasicNode::new("memory://node-1"));

    // `initialize` returns NotAllowed when the node already has Raft state from a
    // previous run.  In that case the cluster is already configured correctly, so
    // skip the bootstrap sequence entirely.
    let already_initialized = leader
      .raft
      .initialize(members)
      .await
      .map(|_| false)
      .or_else(|err| {
        if format!("{err:?}").contains("NotAllowed") {
          Ok(true)
        } else {
          Err(anyhow::anyhow!("initialize raft cluster: {err:?}"))
        }
      })?;

    if already_initialized {
      return Ok(());
    }

    // First-time bootstrap: wait for node 1 to become leader, then add the other
    // two nodes as learners and promote to a 3-voter cluster.
    wait_for_membership_committed(&leader.raft, BTreeSet::from([1]), Duration::from_secs(5))
      .await?;

    for node_id in 2 ..= 3 {
      leader
        .raft
        .add_learner(
          node_id,
          BasicNode::new(format!("memory://node-{node_id}")),
          true, // block until replicated
        )
        .await
        .map_err(|err| anyhow::anyhow!("add learner {node_id}: {err:?}"))?;
      // add_learner(block=true) already awaits replication — no extra wait needed.
    }

    leader
      .raft
      .change_membership(BTreeSet::from([1, 2, 3]), false)
      .await
      .map_err(|err| anyhow::anyhow!("change membership: {err:?}"))?;
    wait_for_membership_committed(
      &leader.raft,
      BTreeSet::from([1, 2, 3]),
      Duration::from_secs(10),
    )
    .await?;
    Ok(())
  }

  async fn wait_for_leader(&self, timeout: Duration) -> anyhow::Result<()> {
    let started = tokio::time::Instant::now();
    while started.elapsed() < timeout {
      if self.leader_id().await.is_ok() {
        return Ok(());
      }
      tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(anyhow::anyhow!("no raft leader after {timeout:?}"))
  }

  async fn leader_id(&self) -> anyhow::Result<u64> {
    let mut tasks = JoinSet::new();
    for (node_id, node) in &self.nodes {
      let node_id = *node_id;
      let raft = node.raft.clone();
      tasks.spawn(async move {
        let metrics = raft.metrics().borrow_watched().clone();
        metrics.state.is_leader().then_some(node_id)
      });
    }

    while let Some(result) = tasks.join_next().await {
      if let Some(node_id) = result.context("join leader probe")? {
        return Ok(node_id);
      }
    }

    Err(anyhow::anyhow!("leader not available"))
  }

  fn node(&self, node_id: u64) -> Option<LocalNode> {
    self.nodes.get(&node_id).cloned()
  }
}

async fn wait_for_membership_committed(
  raft: &Raft,
  expected_voters: BTreeSet<u64>,
  timeout: Duration,
) -> anyhow::Result<()> {
  let started = tokio::time::Instant::now();
  while started.elapsed() < timeout {
    let metrics = raft.metrics().borrow_watched().clone();
    let committed_voters = metrics
      .committed_membership_config
      .voter_ids()
      .collect::<BTreeSet<_>>();
    if metrics.membership_config == metrics.committed_membership_config
      && committed_voters == expected_voters
    {
      return Ok(());
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
  }
  Err(anyhow::anyhow!(
    "membership change did not commit within {timeout:?}"
  ))
}

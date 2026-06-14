mod local_backend;
mod model;
mod network;
mod store;
mod worker;

use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use libp2p::Multiaddr;
use model::{DistributedTask, TaskStatus};
use network::{NetworkCommand, NetworkRole, PeerAddress, spawn_network, submit_task_to_scheduler};
use store::TaskStore;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(about = "A small libp2p + apalis + RocksDB distributed task demo")]
struct Cli {
  #[command(subcommand)]
  command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
  /// Run the scheduler that assigns submitted tasks to workers.
  Scheduler {
    /// RocksDB directory for scheduler state.
    #[arg(long, default_value = "./data/scheduler")]
    db: PathBuf,
    /// libp2p listen address.
    #[arg(long, default_value = "/ip4/127.0.0.1/tcp/7000")]
    listen: Multiaddr,
    /// How many demo tasks to create at startup.
    #[arg(long, default_value_t = 0)]
    tasks: u64,
    /// Delay between task submissions in milliseconds.
    #[arg(long, default_value_t = 1000)]
    interval_ms: u64,
  },
  /// Run a worker that receives tasks and executes them through apalis.
  Worker {
    /// RocksDB directory for worker state.
    #[arg(long, default_value = "./data/worker")]
    db: PathBuf,
    /// Friendly worker name stored in task results.
    #[arg(long, default_value = "worker")]
    name: String,
    /// libp2p listen address.
    #[arg(long, default_value = "/ip4/127.0.0.1/tcp/0")]
    listen: Multiaddr,
    /// Scheduler address formatted as <peer_id>@<multiaddr>.
    #[arg(long)]
    scheduler: PeerAddress,
  },
  /// Submit one task to a running scheduler over libp2p.
  Submit {
    /// Scheduler address formatted as <peer_id>@<multiaddr>.
    #[arg(long)]
    scheduler: PeerAddress,
    /// Task payload to store and process.
    #[arg(long)]
    payload: String,
    /// Optional stable task id. A generated id is used when omitted.
    #[arg(long)]
    task_id: Option<String>,
  },
  /// Print a task record from a RocksDB directory.
  Show {
    /// RocksDB directory.
    #[arg(long)]
    db: PathBuf,
    /// Task id to read.
    #[arg(long)]
    task_id: String,
  },
  /// List task records from a RocksDB directory.
  List {
    /// RocksDB directory.
    #[arg(long)]
    db: PathBuf,
    /// Optional status filter: created, assigned, received, running, completed, failed.
    #[arg(long)]
    status: Option<TaskStatus>,
  },
  /// List currently running task records from a RocksDB directory.
  ListRunning {
    /// RocksDB directory.
    #[arg(long)]
    db: PathBuf,
  },
}

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
    .init();

  match Cli::parse().command {
    Command::Scheduler {
      db,
      listen,
      tasks,
      interval_ms,
    } => run_scheduler(db, listen, tasks, Duration::from_millis(interval_ms)).await,
    Command::Worker {
      db,
      name,
      listen,
      scheduler,
    } => run_worker(db, name, listen, scheduler).await,
    Command::Submit {
      scheduler,
      payload,
      task_id,
    } => submit_task(scheduler, payload, task_id).await,
    Command::Show { db, task_id } => show_record(db, &task_id),
    Command::List { db, status } => list_records(db, status),
    Command::ListRunning { db } => list_running(db),
  }
}

async fn run_scheduler(
  db_path: PathBuf,
  listen: Multiaddr,
  tasks: u64,
  interval: Duration,
) -> Result<()> {
  let store = TaskStore::open(&db_path)?;
  let node = spawn_network(
    NetworkRole::Scheduler,
    listen,
    Vec::new(),
    store.clone(),
    None,
  )
  .await
  .context("starting scheduler network")?;

  info!(peer_id = %node.peer_id, "scheduler started");
  info!("start a worker with --scheduler <peer_id>@<listen-address> from the log above");

  let command_tx = node.command_tx.clone();
  let producer_store = store.clone();
  tokio::spawn(async move {
    let mut ticker = tokio::time::interval(interval);
    for sequence in 1 ..= tasks {
      ticker.tick().await;
      let task = DistributedTask::new(sequence);
      if let Err(err) = producer_store.put_status(&task, TaskStatus::Created, None, None) {
        tracing::error!("{err:#}");
        continue;
      }
      if command_tx.send(NetworkCommand::Submit(task)).await.is_err() {
        break;
      }
    }
  });

  tokio::signal::ctrl_c()
    .await
    .context("waiting for ctrl-c")?;
  Ok(())
}

async fn submit_task(
  scheduler: PeerAddress,
  payload: String,
  task_id: Option<String>,
) -> Result<()> {
  let task = match task_id {
    Some(task_id) => DistributedTask::with_id(task_id, payload),
    None => DistributedTask::from_payload(payload),
  };
  let response = submit_task_to_scheduler(scheduler, task)
    .await
    .context("submitting task to scheduler")?;
  println!("{}", serde_json::to_string_pretty(&response)?);
  Ok(())
}

async fn run_worker(
  db_path: PathBuf,
  name: String,
  listen: Multiaddr,
  scheduler: PeerAddress,
) -> Result<()> {
  let store = TaskStore::open(&db_path)?;
  let worker_tx = worker::spawn_worker(name.clone(), store.clone())
    .await
    .context("starting apalis worker")?;
  let node = spawn_network(
    NetworkRole::Worker,
    listen,
    vec![scheduler],
    store,
    Some(worker_tx),
  )
  .await
  .context("starting worker network")?;

  info!(peer_id = %node.peer_id, %name, "worker started");
  tokio::signal::ctrl_c()
    .await
    .context("waiting for ctrl-c")?;
  Ok(())
}

fn show_record(db_path: PathBuf, task_id: &str) -> Result<()> {
  let store = TaskStore::open_read_only(db_path)?;
  let Some(record) = store.get(task_id)? else {
    println!("task not found");
    return Ok(());
  };
  println!("{}", serde_json::to_string_pretty(&record)?);
  Ok(())
}

fn list_records(db_path: PathBuf, status: Option<TaskStatus>) -> Result<()> {
  let store = TaskStore::open_read_only(db_path)?;
  let records = match status {
    Some(status) => store.list_by_status(status)?,
    None => store.list_all()?,
  };
  println!("{}", serde_json::to_string_pretty(&records)?);
  Ok(())
}

fn list_running(db_path: PathBuf) -> Result<()> {
  let store = TaskStore::open_read_only(db_path)?;
  let records = store.list_active()?;
  println!("{}", serde_json::to_string_pretty(&records)?);
  Ok(())
}

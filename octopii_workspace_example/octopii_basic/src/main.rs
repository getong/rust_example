use std::{
  collections::{HashMap, VecDeque},
  path::{Path, PathBuf},
  sync::{Arc, Mutex},
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use bytes::Bytes;
use octopii::{
  Config, OctopiiError, OctopiiNode, OctopiiRuntime, Result, StateMachine, StateMachineTrait,
};
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;

#[derive(Serialize, Deserialize, Clone, Debug)]
enum TaskCmd {
  Submit {
    task_id: String,
    payload: Vec<u8>,
  },
  Claim {
    worker_id: String,
  },
  Complete {
    task_id: String,
    result: Vec<u8>,
  },
  Fail {
    task_id: String,
    reason: String,
    retry: bool,
  },
  Status {
    task_id: String,
  },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum TaskStatus {
  Pending,
  Assigned { worker: String },
  Done(Vec<u8>),
  Failed(String),
}

#[derive(Default, Serialize, Deserialize)]
struct QueueState {
  tasks: HashMap<String, TaskStatus>,
  pending: VecDeque<String>,
}

struct TaskQueueStateMachine {
  state: Mutex<QueueState>,
}

impl TaskQueueStateMachine {
  fn new() -> Self {
    Self {
      state: Mutex::new(QueueState::default()),
    }
  }
}

impl StateMachineTrait for TaskQueueStateMachine {
  fn apply(&self, command: &[u8]) -> std::result::Result<Bytes, String> {
    let cmd = decode(command)?;
    let mut state = self
      .state
      .lock()
      .map_err(|_| "task queue state lock poisoned".to_string())?;

    match cmd {
      TaskCmd::Submit { task_id, .. } => {
        state.tasks.insert(task_id.clone(), TaskStatus::Pending);
        state.pending.push_back(task_id);
        Ok(Bytes::from_static(b"ok"))
      }
      TaskCmd::Claim { worker_id } => {
        if let Some(task_id) = state.pending.pop_front() {
          state
            .tasks
            .insert(task_id.clone(), TaskStatus::Assigned { worker: worker_id });
          Ok(Bytes::from(task_id))
        } else {
          Ok(Bytes::new())
        }
      }
      TaskCmd::Complete { task_id, result } => {
        state.tasks.insert(task_id, TaskStatus::Done(result));
        Ok(Bytes::from_static(b"ok"))
      }
      TaskCmd::Fail {
        task_id,
        reason,
        retry,
      } => {
        if retry {
          state.tasks.insert(task_id.clone(), TaskStatus::Pending);
          state.pending.push_back(task_id);
        } else {
          state.tasks.insert(task_id, TaskStatus::Failed(reason));
        }
        Ok(Bytes::from_static(b"ok"))
      }
      TaskCmd::Status { task_id } => {
        let status = state
          .tasks
          .get(&task_id)
          .map(|status| format!("{status:?}"))
          .unwrap_or_else(|| "not_found".to_string());
        Ok(Bytes::from(status))
      }
    }
  }

  fn snapshot(&self) -> Vec<u8> {
    self
      .state
      .lock()
      .ok()
      .and_then(|state| bincode::serialize(&*state).ok())
      .unwrap_or_default()
  }

  fn restore(&self, data: &[u8]) -> std::result::Result<(), String> {
    let restored: QueueState = bincode::deserialize(data).map_err(|e| e.to_string())?;
    *self
      .state
      .lock()
      .map_err(|_| "task queue state lock poisoned".to_string())? = restored;
    Ok(())
  }
}

fn encode(cmd: &TaskCmd) -> Vec<u8> {
  bincode::serialize(cmd).expect("TaskCmd serialization should not fail")
}

fn decode(bytes: &[u8]) -> std::result::Result<TaskCmd, String> {
  bincode::deserialize(bytes).map_err(|e| e.to_string())
}

async fn start_task_node(
  id: u64,
  bind: &str,
  peers: &[&str],
  dir: impl AsRef<Path>,
  leader: bool,
) -> Result<Arc<OctopiiNode>> {
  let config = Config {
    node_id: id,
    bind_addr: bind
      .parse()
      .map_err(|e| OctopiiError::Rpc(format!("invalid bind address {bind}: {e}")))?,
    peers: peers
      .iter()
      .map(|addr| {
        addr
          .parse()
          .map_err(|e| OctopiiError::Rpc(format!("invalid peer address {addr}: {e}")))
      })
      .collect::<Result<Vec<_>>>()?,
    wal_dir: dir.as_ref().into(),
    is_initial_leader: leader,
    ..Default::default()
  };
  let state_machine: StateMachine = Arc::new(TaskQueueStateMachine::new());
  let node = OctopiiNode::new_with_state_machine(
    config,
    OctopiiRuntime::from_handle(Handle::current()),
    state_machine,
  )
  .await?;
  node.set_election_enabled(false);
  node.start().await?;
  Ok(Arc::new(node))
}

async fn worker_loop(node: &OctopiiNode, worker_id: &str) {
  loop {
    let claim = encode(&TaskCmd::Claim {
      worker_id: worker_id.to_string(),
    });
    let task_id = match node.propose(claim).await {
      Ok(response) if response.is_empty() => {
        tokio::time::sleep(Duration::from_millis(100)).await;
        continue;
      }
      Ok(response) => String::from_utf8_lossy(&response).to_string(),
      Err(error) => {
        eprintln!("claim failed: {error}");
        tokio::time::sleep(Duration::from_millis(200)).await;
        continue;
      }
    };

    let done = match execute_task(&task_id).await {
      Ok(result) => encode(&TaskCmd::Complete { task_id, result }),
      Err(reason) => encode(&TaskCmd::Fail {
        task_id,
        reason,
        retry: true,
      }),
    };

    if let Err(error) = node.propose(done).await {
      eprintln!("complete failed: {error}");
    }
    break;
  }
}

async fn execute_task(task_id: &str) -> std::result::Result<Vec<u8>, String> {
  tokio::time::sleep(Duration::from_millis(50)).await;
  Ok(format!("result-of-{task_id}").into_bytes())
}

async fn wait_until_leader(node: &OctopiiNode, timeout: Duration) -> Result<()> {
  let started = tokio::time::Instant::now();
  while started.elapsed() < timeout {
    if node.is_leader().await {
      return Ok(());
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
  }

  let metrics = node.raft_metrics();
  Err(OctopiiError::Rpc(format!(
    "node {} did not become leader within {:?}; state={:?}, leader={:?}",
    node.id(),
    timeout,
    metrics.state,
    metrics.current_leader
  )))
}

#[tokio::main]
async fn main() -> Result<()> {
  let run_id = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_err(|e| OctopiiError::Rpc(format!("system clock before unix epoch: {e}")))?
    .as_millis();
  let data_dir =
    PathBuf::from("target/octopii-basic-data").join(format!("run-{run_id}-{}", std::process::id()));
  let n1 = start_task_node(
    1,
    "127.0.0.1:5001",
    &["127.0.0.1:5002", "127.0.0.1:5003"],
    data_dir.join("n1"),
    true,
  )
  .await?;
  let n2 = start_task_node(
    2,
    "127.0.0.1:5002",
    &["127.0.0.1:5001", "127.0.0.1:5003"],
    data_dir.join("n2"),
    false,
  )
  .await?;
  let n3 = start_task_node(
    3,
    "127.0.0.1:5003",
    &["127.0.0.1:5001", "127.0.0.1:5002"],
    data_dir.join("n3"),
    false,
  )
  .await?;

  if !n1.is_leader().await {
    n1.campaign().await?;
  }
  wait_until_leader(&n1, Duration::from_secs(5)).await?;

  n1.propose(encode(&TaskCmd::Submit {
    task_id: "job-001".into(),
    payload: b"process video.mp4".to_vec(),
  }))
  .await?;

  let n1_worker = Arc::clone(&n1);
  let worker = tokio::spawn(async move { worker_loop(&n1_worker, "worker-A").await });
  worker
    .await
    .map_err(|e| OctopiiError::Rpc(format!("worker task failed: {e}")))?;

  tokio::time::sleep(Duration::from_millis(300)).await;

  let query = encode(&TaskCmd::Status {
    task_id: "job-001".into(),
  });
  let status = n2.query(&query).await?;
  println!("job-001: {}", String::from_utf8_lossy(&status));

  let metrics = n1.raft_metrics();
  println!(
    "Leader={:?}, LastApplied={:?}",
    metrics.current_leader, metrics.last_applied
  );

  n3.shutdown().await;
  n2.shutdown().await;
  n1.shutdown().await;

  Ok(())
}

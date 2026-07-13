//! `olpc-task`: task-queue client for the control + worker cluster.
//!
//! Talks to any node's HTTP endpoint (control nodes write through their
//! local raft handle; worker nodes forward over the tarpc TaskRpc). Used by
//! `script/task-client-test.sh` to verify the architecture end to end.

use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use anyhow::{Context, anyhow};
use clap::{Parser, Subcommand};
use openraft_libp2p_cluster::tasks::{
  TaskQueueMetrics, TaskRecord, WorkerLeaseRecord,
  handlers::{TaskPayload, WasmExec},
};
use serde::Deserialize;

#[derive(Parser)]
#[command(
  author,
  version,
  about = "Task queue client for the openraft+libp2p cluster"
)]
struct Opt {
  /// HTTP address of any cluster node (control or worker).
  #[arg(long, global = true, default_value = "127.0.0.1:3001")]
  http: String,

  #[command(subcommand)]
  cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
  /// Enqueue email task(s).
  Push {
    /// Recipient. Prefixes trigger drill behavior on workers:
    /// `fail*` simulates handler errors, `slow*` simulates a hang.
    #[arg(long)]
    to: String,
    /// Number of tasks to push (recipient gets a -<n> suffix when > 1).
    #[arg(long, default_value_t = 1)]
    count: u32,
    /// Idempotency key; a repeated push with the same key is deduplicated.
    #[arg(long)]
    idem: Option<String>,
  },
  /// Enqueue task(s) of any kind from a raw kind-tagged JSON payload, e.g.
  /// '{"kind":"digest","data":"abc","iterations":10000}'. Kinds are the
  /// TaskPayload enum variants: email, webhook, digest, kv_set, sleep, wasm
  /// (wasm carries the handler itself: module_wat or module_b64 + args/env).
  PushTask {
    /// Kind-tagged JSON payload (a TaskPayload variant).
    #[arg(long)]
    payload: String,
    /// Number of copies to push.
    #[arg(long, default_value_t = 1)]
    count: u32,
    /// Idempotency key; a repeated push with the same key is deduplicated.
    #[arg(long)]
    idem: Option<String>,
    /// Schedule the task this many seconds into the future.
    #[arg(long, default_value_t = 0)]
    delay_secs: u64,
  },
  /// Enqueue a wasm task whose HANDLER travels inside the payload
  /// (code-as-data): the module file — WAT text or a compiled wasm
  /// binary — is read here and stored with the task in the raft log; a
  /// worker executes it on the WASI p2 component runtime (wasmtime) and
  /// stores its stdout as the task result.
  PushWasm {
    /// Human-readable WAT text module file.
    #[arg(
      long,
      conflicts_with = "wasm_file",
      required_unless_present = "wasm_file"
    )]
    wat_file: Option<PathBuf>,
    /// Compiled wasm binary module file (base64-encoded into the
    /// payload): a p2 component (wasm32-wasip2) or a p1 core module
    /// (wasm32-wasip1) — p1 modules are auto-adapted server-side.
    #[arg(long)]
    wasm_file: Option<PathBuf>,
    /// Display name for lists (defaults to the module file stem).
    #[arg(long)]
    name: Option<String>,
    /// argv[1..] for the guest (repeatable).
    #[arg(long = "arg")]
    args: Vec<String>,
    /// KEY=VALUE guest environment variable (repeatable).
    #[arg(long = "env")]
    env: Vec<String>,
    /// Idempotency key; a repeated push with the same key is deduplicated.
    #[arg(long)]
    idem: Option<String>,
    /// Schedule the task this many seconds into the future.
    #[arg(long, default_value_t = 0)]
    delay_secs: u64,
  },
  /// List task records.
  List {
    /// Filter by status: queued|assigned|running|done|failed.
    #[arg(long)]
    status: Option<String>,
  },
  /// List worker leases.
  Workers,
  /// Queue health metrics.
  Metrics,
  /// Poll until no task is in flight (or timeout), then verify expectations.
  Watch {
    #[arg(long, default_value_t = 120)]
    timeout_secs: u64,
    /// Expected number of Done tasks (verified after settling).
    #[arg(long)]
    expect_done: Option<usize>,
    /// Expected number of Failed tasks.
    #[arg(long)]
    expect_failed: Option<usize>,
    /// Expected total number of task records.
    #[arg(long)]
    expect_total: Option<usize>,
    /// Require every Done task to have exactly one attempt
    /// (exactly-once check for tasks that are not retry drills).
    #[arg(long, default_value_t = false)]
    expect_single_attempt_done: bool,
    /// Require Done tasks to be spread over at least this many workers.
    #[arg(long)]
    min_distinct_workers: Option<usize>,
  },
}

#[derive(Deserialize)]
struct EmailResponse {
  ok: bool,
  task_id: Option<String>,
  deduplicated: Option<bool>,
  error: Option<String>,
}

#[derive(Deserialize)]
struct TasksResponse {
  ok: bool,
  tasks: Vec<TaskRecord>,
  error: Option<String>,
}

#[derive(Deserialize)]
struct WorkersResponse {
  ok: bool,
  workers: Vec<WorkerLeaseRecord>,
  error: Option<String>,
}

#[derive(Deserialize)]
struct MetricsResponse {
  ok: bool,
  metrics: Option<TaskQueueMetrics>,
  error: Option<String>,
}

struct Client {
  base: String,
  http: reqwest::Client,
}

impl Client {
  fn new(addr: &str) -> anyhow::Result<Self> {
    Ok(Self {
      base: format!("http://{addr}"),
      http: reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?,
    })
  }

  async fn push(&self, to: &str, idem: Option<&str>) -> anyhow::Result<EmailResponse> {
    let mut body = BTreeMap::new();
    body.insert("to", to.to_string());
    if let Some(idem) = idem {
      body.insert("idem_key", idem.to_string());
    }
    let response: EmailResponse = self
      .http
      .post(format!("{}/tasks/email", self.base))
      .json(&body)
      .send()
      .await
      .context("push request failed")?
      .json()
      .await
      .context("decode push response")?;
    if !response.ok {
      return Err(anyhow!(
        "push rejected: {}",
        response.error.clone().unwrap_or_default()
      ));
    }
    Ok(response)
  }

  /// Generic multi-kind submission via /tasks/push.
  async fn push_task(
    &self,
    payload: &sonic_rs::Value,
    idem: Option<&str>,
    delay_secs: u64,
  ) -> anyhow::Result<EmailResponse> {
    let body = sonic_rs::json!({
      "payload": payload,
      "idem_key": idem,
      "delay_secs": delay_secs,
    });
    let response: EmailResponse = self
      .http
      .post(format!("{}/tasks/push", self.base))
      .header("content-type", "application/json")
      .body(sonic_rs::to_string(&body).context("encode push-task body")?)
      .send()
      .await
      .context("push-task request failed")?
      .json()
      .await
      .context("decode push-task response")?;
    if !response.ok {
      return Err(anyhow!(
        "push-task rejected: {}",
        response.error.clone().unwrap_or_default()
      ));
    }
    Ok(response)
  }

  async fn tasks(&self) -> anyhow::Result<Vec<TaskRecord>> {
    let response: TasksResponse = self
      .http
      .get(format!("{}/tasks", self.base))
      .send()
      .await
      .context("list request failed")?
      .json()
      .await
      .context("decode list response")?;
    if !response.ok {
      return Err(anyhow!(
        "list rejected: {}",
        response.error.unwrap_or_default()
      ));
    }
    Ok(response.tasks)
  }

  async fn workers(&self) -> anyhow::Result<Vec<WorkerLeaseRecord>> {
    let response: WorkersResponse = self
      .http
      .get(format!("{}/tasks/workers", self.base))
      .send()
      .await
      .context("workers request failed")?
      .json()
      .await
      .context("decode workers response")?;
    if !response.ok {
      return Err(anyhow!(
        "workers rejected: {}",
        response.error.unwrap_or_default()
      ));
    }
    Ok(response.workers)
  }

  async fn metrics(&self) -> anyhow::Result<TaskQueueMetrics> {
    let response: MetricsResponse = self
      .http
      .get(format!("{}/tasks/metrics", self.base))
      .send()
      .await
      .context("metrics request failed")?
      .json()
      .await
      .context("decode metrics response")?;
    match response.metrics {
      Some(metrics) if response.ok => Ok(metrics),
      _ => Err(anyhow!(
        "metrics rejected: {}",
        response.error.unwrap_or_default()
      )),
    }
  }
}

/// Compact `kind:detail` label for a task payload, straight from the
/// [`TaskPayload`] enum (untagged payloads are the legacy email shape).
fn task_label(record: &TaskRecord) -> String {
  TaskPayload::decode(&record.payload)
    .map(|payload| payload.label())
    .unwrap_or_else(|_| "<unknown>".to_string())
}

fn print_tasks(tasks: &[TaskRecord]) {
  println!(
    "{:<24} {:<9} {:<8} {:<10} {:<40} {}",
    "TASK", "STATUS", "ATTEMPTS", "WORKER", "RESULT", "ERROR"
  );
  for task in tasks {
    let mut result = task.result.clone().unwrap_or_else(|| "-".to_string());
    if result.len() > 40 {
      result.truncate(37);
      result.push_str("...");
    }
    println!(
      "{:<24} {:<9} {:<8} {:<10} {:<40} {}",
      task_label(task),
      task.status.as_str(),
      task.attempts,
      task
        .assigned_node_id
        .as_deref()
        .map(|node| &node[node.len().saturating_sub(8) ..])
        .unwrap_or("-"),
      result,
      task.error.as_deref().unwrap_or("-")
    );
  }
}

fn is_in_flight(record: &TaskRecord) -> bool {
  use openraft_libp2p_cluster::tasks::TaskStatus;
  matches!(
    record.status,
    TaskStatus::Queued | TaskStatus::Assigned | TaskStatus::Running
  )
}

#[allow(clippy::too_many_arguments)]
async fn watch(
  client: &Client,
  timeout_secs: u64,
  expect_done: Option<usize>,
  expect_failed: Option<usize>,
  expect_total: Option<usize>,
  expect_single_attempt_done: bool,
  min_distinct_workers: Option<usize>,
) -> anyhow::Result<()> {
  use openraft_libp2p_cluster::tasks::TaskStatus;

  let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
  let tasks = loop {
    let tasks = client.tasks().await?;
    let in_flight = tasks.iter().filter(|task| is_in_flight(task)).count();
    if in_flight == 0 {
      break tasks;
    }
    if tokio::time::Instant::now() >= deadline {
      print_tasks(&tasks);
      return Err(anyhow!(
        "{in_flight} task(s) still in flight after {timeout_secs}s"
      ));
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
  };

  print_tasks(&tasks);

  let done: Vec<&TaskRecord> = tasks
    .iter()
    .filter(|task| task.status == TaskStatus::Done)
    .collect();
  let failed = tasks
    .iter()
    .filter(|task| task.status == TaskStatus::Failed)
    .count();

  let mut failures = Vec::new();
  if let Some(expected) = expect_total
    && tasks.len() != expected
  {
    failures.push(format!("total: expected {expected}, got {}", tasks.len()));
  }
  if let Some(expected) = expect_done
    && done.len() != expected
  {
    failures.push(format!("done: expected {expected}, got {}", done.len()));
  }
  if let Some(expected) = expect_failed
    && failed != expected
  {
    failures.push(format!("failed: expected {expected}, got {failed}"));
  }
  if expect_single_attempt_done && let Some(task) = done.iter().find(|task| task.attempts != 1) {
    failures.push(format!(
      "task {} is Done with attempts={}, expected exactly 1",
      task.id, task.attempts
    ));
  }
  if let Some(minimum) = min_distinct_workers {
    let distinct: std::collections::BTreeSet<&str> = done
      .iter()
      .filter_map(|task| task.assigned_node_id.as_deref())
      .collect();
    if distinct.len() < minimum {
      failures.push(format!(
        "distinct workers for Done tasks: expected >= {minimum}, got {}",
        distinct.len()
      ));
    }
  }

  println!(
    "settled: total={} done={} failed={}",
    tasks.len(),
    done.len(),
    failed
  );

  if failures.is_empty() {
    println!("VERIFY: PASS");
    Ok(())
  } else {
    for failure in &failures {
      eprintln!("VERIFY FAIL: {failure}");
    }
    Err(anyhow!("{} expectation(s) failed", failures.len()))
  }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let opt = Opt::parse();
  let client = Client::new(&opt.http)?;

  match opt.cmd {
    Cmd::Push { to, count, idem } => {
      for index in 1 ..= count {
        let recipient = if count > 1 {
          format!("{to}-{index}")
        } else {
          to.clone()
        };
        let response = client.push(&recipient, idem.as_deref()).await?;
        println!(
          "pushed to={} task_id={} deduplicated={}",
          recipient,
          response.task_id.as_deref().unwrap_or("-"),
          response.deduplicated.unwrap_or(false)
        );
      }
    }
    Cmd::PushTask {
      payload,
      count,
      idem,
      delay_secs,
    } => {
      let payload: sonic_rs::Value =
        sonic_rs::from_str(&payload).context("--payload must be valid JSON")?;
      for _ in 1 ..= count {
        let response = client
          .push_task(&payload, idem.as_deref(), delay_secs)
          .await?;
        println!(
          "pushed payload={} task_id={} deduplicated={}",
          sonic_rs::to_string(&payload)?,
          response.task_id.as_deref().unwrap_or("-"),
          response.deduplicated.unwrap_or(false)
        );
      }
    }
    Cmd::PushWasm {
      wat_file,
      wasm_file,
      name,
      args,
      env,
      idem,
      delay_secs,
    } => {
      let (module_wat, module_b64, module_path) = match (&wat_file, &wasm_file) {
        (Some(path), None) => {
          let wat = std::fs::read_to_string(path)
            .with_context(|| format!("read wat module {}", path.display()))?;
          (Some(wat), None, path.clone())
        }
        (None, Some(path)) => {
          use base64::Engine as _;
          let bytes =
            std::fs::read(path).with_context(|| format!("read wasm module {}", path.display()))?;
          let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
          (None, Some(b64), path.clone())
        }
        _ => return Err(anyhow!("pass exactly one of --wat-file / --wasm-file")),
      };
      let name = name.or_else(|| {
        module_path
          .file_stem()
          .map(|stem| stem.to_string_lossy().into_owned())
      });
      let mut env_pairs = BTreeMap::new();
      for pair in env {
        let (key, value) = pair
          .split_once('=')
          .ok_or_else(|| anyhow!("--env expects KEY=VALUE, got {pair:?}"))?;
        env_pairs.insert(key.to_string(), value.to_string());
      }

      let payload = TaskPayload::Wasm(WasmExec {
        module_wat,
        module_b64,
        args,
        env: env_pairs,
        name,
      })
      .encode()
      .map_err(|err| anyhow!("encode wasm payload: {err}"))?;
      let payload: sonic_rs::Value = sonic_rs::from_str(&payload)?;
      let response = client
        .push_task(&payload, idem.as_deref(), delay_secs)
        .await?;
      println!(
        "pushed wasm module={} bytes={} task_id={} deduplicated={}",
        module_path.display(),
        std::fs::metadata(&module_path)
          .map(|m| m.len())
          .unwrap_or(0),
        response.task_id.as_deref().unwrap_or("-"),
        response.deduplicated.unwrap_or(false)
      );
    }
    Cmd::List { status } => {
      let mut tasks = client.tasks().await?;
      if let Some(filter) = status {
        tasks.retain(|task| task.status.as_str().eq_ignore_ascii_case(&filter));
      }
      print_tasks(&tasks);
    }
    Cmd::Workers => {
      let workers = client.workers().await?;
      println!("{:<55} {:<12} {}", "NODE", "LEASE_EPOCH", "EXPIRES_AT");
      for worker in workers {
        println!(
          "{:<55} {:<12} {}",
          worker.node_id, worker.lease_epoch, worker.expires_at
        );
      }
    }
    Cmd::Metrics => {
      let metrics = client.metrics().await?;
      println!("{}", sonic_rs::to_string(&metrics)?);
    }
    Cmd::Watch {
      timeout_secs,
      expect_done,
      expect_failed,
      expect_total,
      expect_single_attempt_done,
      min_distinct_workers,
    } => {
      watch(
        &client,
        timeout_secs,
        expect_done,
        expect_failed,
        expect_total,
        expect_single_attempt_done,
        min_distinct_workers,
      )
      .await?;
    }
  }

  Ok(())
}

//! Task-native replicated state machine logic (octopii-style port).
//!
//! A task is a domain command in the raft log. `apply_task_command` performs
//! every state transition ATOMICALLY inside the state machine's apply step:
//! the task record, its secondary indexes (queued / assigned), the
//! idempotency table, and worker leases all change in the same write batch.
//!
//! Two invariants:
//!   - Determinism: this module never reads the clock or randomness; all timestamps come from the
//!     proposed command.
//!   - No execution in apply(): tasks are DATA here. Side effects run on exactly one worker via the
//!     claim/lease protocol (see [`crate::tasks::worker`]).

pub mod rpc;
pub mod scheduler;
pub mod worker;

use serde::{Deserialize, Serialize};

use crate::types_kv::{Request, Response};

/// Maximum executions per task before it is marked failed permanently.
pub const MAX_TASK_ATTEMPTS: u32 = 3;

pub const TASK_REC_PREFIX: &str = "task:rec:";
pub const TASK_QUEUED_IDX_PREFIX: &str = "task:idx:queued:";
pub const TASK_ASSIGNED_IDX_PREFIX: &str = "task:idx:assigned:";
pub const TASK_IDEM_PREFIX: &str = "task:idem:";
pub const TASK_WORKER_PREFIX: &str = "task:worker:";

pub fn rec_key(id: &str) -> String {
  format!("{TASK_REC_PREFIX}{id}")
}

/// Zero-padded run_at keeps the queued index sorted by due time, so the
/// scheduler reads ready tasks with one narrow prefix scan.
pub fn queued_idx_key(run_at: u64, id: &str) -> String {
  format!("{TASK_QUEUED_IDX_PREFIX}{run_at:020}:{id}")
}

pub fn assigned_idx_key(node_id: &str, id: &str) -> String {
  format!("{TASK_ASSIGNED_IDX_PREFIX}{node_id}:{id}")
}

pub fn assigned_idx_node_prefix(node_id: &str) -> String {
  format!("{TASK_ASSIGNED_IDX_PREFIX}{node_id}:")
}

pub fn idem_record_key(idem_key: &str) -> String {
  format!("{TASK_IDEM_PREFIX}{idem_key}")
}

pub fn worker_key(node_id: &str) -> String {
  format!("{TASK_WORKER_PREFIX}{node_id}")
}

/// Parse `task:idx:queued:{run_at:020}:{id}` → (run_at, id).
pub fn parse_queued_idx_key(key: &str) -> Option<(u64, &str)> {
  let rest = key.strip_prefix(TASK_QUEUED_IDX_PREFIX)?;
  let (run_at, id) = rest.split_once(':')?;
  Some((run_at.parse().ok()?, id))
}

/// Parse `task:idx:assigned:{node_id}:{id}` → (node_id, id).
pub fn parse_assigned_idx_key(key: &str) -> Option<(&str, &str)> {
  let rest = key.strip_prefix(TASK_ASSIGNED_IDX_PREFIX)?;
  // node ids never contain ':'; task ids are UUIDs.
  rest.rsplit_once(':')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
  Queued,
  Assigned,
  Running,
  Done,
  Failed,
}

impl TaskStatus {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Queued => "queued",
      Self::Assigned => "assigned",
      Self::Running => "running",
      Self::Done => "done",
      Self::Failed => "failed",
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
  pub id: String,
  /// Serialized task arguments (opaque JSON).
  pub payload: String,
  pub status: TaskStatus,
  pub attempts: u32,
  pub run_at: u64,
  pub idem_key: Option<String>,
  pub assigned_node_id: Option<String>,
  pub lease_epoch: Option<u64>,
  pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerLeaseRecord {
  pub node_id: String,
  pub worker_name: String,
  pub lease_epoch: u64,
  pub expires_at: u64,
}

/// One key mutation produced by applying a task command. `None` deletes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvMutation {
  pub key: String,
  pub value: Option<String>,
}

impl KvMutation {
  fn put(key: String, value: String) -> Self {
    Self {
      key,
      value: Some(value),
    }
  }

  fn del(key: String) -> Self {
    Self { key, value: None }
  }
}

/// Structured apply result carried back to the proposer in
/// `Response::value` as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOpResult {
  pub ok: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub deduplicated: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub record: Option<TaskRecord>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
}

impl TaskOpResult {
  fn ok() -> Self {
    Self {
      ok: true,
      id: None,
      deduplicated: None,
      record: None,
      reason: None,
    }
  }

  fn rejected(reason: impl Into<String>) -> Self {
    Self {
      ok: false,
      id: None,
      deduplicated: None,
      record: None,
      reason: Some(reason.into()),
    }
  }

  fn into_response(self) -> Response {
    Response {
      value: sonic_rs::to_string(&self).ok(),
    }
  }

  pub fn from_response(response: &Response) -> Option<Self> {
    sonic_rs::from_str(response.value.as_deref()?).ok()
  }
}

/// A read of the CURRENT state-machine value for a key. Injected so the
/// logic stays storage-agnostic (RocksDB in production, a map in tests).
pub type StateRead<'a> = dyn FnMut(&str) -> Result<Option<String>, String> + 'a;

/// Returns `Some` when `cmd` is a task command this module owns.
pub fn is_task_command(cmd: &Request) -> bool {
  !matches!(cmd, Request::Set { .. } | Request::Delete { .. })
}

/// Deterministically apply one task command against the current state.
/// Returns the key mutations (applied atomically by the caller in the same
/// write batch) and the proposer-visible response.
pub fn apply_task_command(
  read: &mut StateRead<'_>,
  cmd: Request,
) -> Result<(Vec<KvMutation>, Response), String> {
  match cmd {
    Request::TaskEnqueue {
      id,
      payload,
      run_at,
      idem_key,
    } => apply_enqueue(read, id, payload, run_at, idem_key),
    Request::TaskAssign {
      id,
      node_id,
      lease_epoch,
    } => apply_assign(read, id, node_id, lease_epoch),
    Request::TaskClaim {
      id,
      node_id,
      lease_epoch,
    } => apply_claim(read, id, node_id, lease_epoch),
    Request::TaskDone {
      id,
      node_id,
      lease_epoch,
      attempts,
    } => apply_done(read, id, node_id, lease_epoch, attempts),
    Request::TaskFail {
      id,
      node_id,
      lease_epoch,
      attempts,
      error,
      retry_at,
    } => apply_fail(read, id, node_id, lease_epoch, attempts, error, retry_at),
    Request::TaskRequeue { id } => apply_requeue(read, id),
    Request::WorkerLease {
      node_id,
      worker_name,
      lease_epoch,
      expires_at,
    } => apply_worker_lease(node_id, worker_name, lease_epoch, expires_at),
    Request::Set { .. } | Request::Delete { .. } => {
      Err("generic KV commands are not task commands".to_string())
    }
  }
}

fn read_record(read: &mut StateRead<'_>, id: &str) -> Result<Option<TaskRecord>, String> {
  let Some(raw) = read(&rec_key(id))? else {
    return Ok(None);
  };
  sonic_rs::from_str(&raw).map_err(|err| format!("corrupt task record {id}: {err}"))
}

fn encode_record(record: &TaskRecord) -> Result<String, String> {
  sonic_rs::to_string(record).map_err(|err| format!("encode task record: {err}"))
}

fn apply_enqueue(
  read: &mut StateRead<'_>,
  id: String,
  payload: String,
  run_at: u64,
  idem_key: Option<String>,
) -> Result<(Vec<KvMutation>, Response), String> {
  // Idempotency: an existing key wins; return the original id, write nothing.
  if let Some(idem) = idem_key.as_deref()
    && let Some(existing_id) = read(&idem_record_key(idem))?
  {
    let result = TaskOpResult {
      ok: true,
      id: Some(existing_id),
      deduplicated: Some(true),
      record: None,
      reason: None,
    };
    return Ok((Vec::new(), result.into_response()));
  }

  // Re-proposing the same id is a no-op (raft retries).
  if read_record(read, &id)?.is_some() {
    let result = TaskOpResult {
      ok: true,
      id: Some(id),
      deduplicated: Some(true),
      record: None,
      reason: None,
    };
    return Ok((Vec::new(), result.into_response()));
  }

  let record = TaskRecord {
    id: id.clone(),
    payload,
    status: TaskStatus::Queued,
    attempts: 0,
    run_at,
    idem_key: idem_key.clone(),
    assigned_node_id: None,
    lease_epoch: None,
    error: None,
  };

  let mut mutations = vec![
    KvMutation::put(rec_key(&id), encode_record(&record)?),
    KvMutation::put(queued_idx_key(run_at, &id), id.clone()),
  ];
  if let Some(idem) = idem_key {
    mutations.push(KvMutation::put(idem_record_key(&idem), id.clone()));
  }

  let result = TaskOpResult {
    ok: true,
    id: Some(id),
    deduplicated: Some(false),
    record: None,
    reason: None,
  };
  Ok((mutations, result.into_response()))
}

fn apply_assign(
  read: &mut StateRead<'_>,
  id: String,
  node_id: String,
  lease_epoch: u64,
) -> Result<(Vec<KvMutation>, Response), String> {
  let Some(mut record) = read_record(read, &id)? else {
    return Ok((
      Vec::new(),
      TaskOpResult::rejected("task not found").into_response(),
    ));
  };
  if record.status != TaskStatus::Queued {
    return Ok((
      Vec::new(),
      TaskOpResult::rejected(format!("task is {}, not queued", record.status.as_str()))
        .into_response(),
    ));
  }

  let queued_key = queued_idx_key(record.run_at, &id);
  record.status = TaskStatus::Assigned;
  record.assigned_node_id = Some(node_id.clone());
  record.lease_epoch = Some(lease_epoch);

  let mutations = vec![
    KvMutation::put(rec_key(&id), encode_record(&record)?),
    KvMutation::del(queued_key),
    KvMutation::put(assigned_idx_key(&node_id, &id), id.clone()),
  ];
  Ok((mutations, TaskOpResult::ok().into_response()))
}

fn apply_claim(
  read: &mut StateRead<'_>,
  id: String,
  node_id: String,
  lease_epoch: u64,
) -> Result<(Vec<KvMutation>, Response), String> {
  let Some(mut record) = read_record(read, &id)? else {
    return Ok((
      Vec::new(),
      TaskOpResult::rejected("task not found").into_response(),
    ));
  };

  if record.status != TaskStatus::Assigned
    || record.assigned_node_id.as_deref() != Some(node_id.as_str())
    || record.lease_epoch != Some(lease_epoch)
  {
    return Ok((
      Vec::new(),
      TaskOpResult::rejected(format!(
        "claim mismatch: status={}, assigned={:?}, lease={:?}",
        record.status.as_str(),
        record.assigned_node_id,
        record.lease_epoch
      ))
      .into_response(),
    ));
  }

  record.status = TaskStatus::Running;
  record.attempts = record.attempts.saturating_add(1);
  let mutations = vec![KvMutation::put(rec_key(&id), encode_record(&record)?)];
  let result = TaskOpResult {
    ok: true,
    id: Some(id),
    deduplicated: None,
    record: Some(record),
    reason: None,
  };
  Ok((mutations, result.into_response()))
}

fn ack_matches(record: &TaskRecord, node_id: &str, lease_epoch: u64) -> bool {
  record.status == TaskStatus::Running
    && record.assigned_node_id.as_deref() == Some(node_id)
    && record.lease_epoch == Some(lease_epoch)
}

fn apply_done(
  read: &mut StateRead<'_>,
  id: String,
  node_id: String,
  lease_epoch: u64,
  attempts: u32,
) -> Result<(Vec<KvMutation>, Response), String> {
  let Some(mut record) = read_record(read, &id)? else {
    return Ok((
      Vec::new(),
      TaskOpResult::rejected("task not found").into_response(),
    ));
  };
  if !ack_matches(&record, &node_id, lease_epoch) {
    return Ok((
      Vec::new(),
      TaskOpResult::rejected("stale ack ignored").into_response(),
    ));
  }

  let assigned_key = assigned_idx_key(&node_id, &id);
  record.status = TaskStatus::Done;
  record.attempts = attempts;
  record.error = None;

  let mutations = vec![
    KvMutation::put(rec_key(&id), encode_record(&record)?),
    KvMutation::del(assigned_key),
  ];
  Ok((mutations, TaskOpResult::ok().into_response()))
}

fn apply_fail(
  read: &mut StateRead<'_>,
  id: String,
  node_id: String,
  lease_epoch: u64,
  attempts: u32,
  error: String,
  retry_at: u64,
) -> Result<(Vec<KvMutation>, Response), String> {
  let Some(mut record) = read_record(read, &id)? else {
    return Ok((
      Vec::new(),
      TaskOpResult::rejected("task not found").into_response(),
    ));
  };
  if !ack_matches(&record, &node_id, lease_epoch) {
    return Ok((
      Vec::new(),
      TaskOpResult::rejected("stale ack ignored").into_response(),
    ));
  }

  let assigned_key = assigned_idx_key(&node_id, &id);
  record.attempts = attempts;
  record.error = Some(error);
  record.assigned_node_id = None;
  record.lease_epoch = None;

  let mut mutations = vec![KvMutation::del(assigned_key)];
  if retry_at > 0 {
    // Delayed retry: back to the queue with the new due time.
    record.status = TaskStatus::Queued;
    record.run_at = retry_at;
    mutations.push(KvMutation::put(queued_idx_key(retry_at, &id), id.clone()));
  } else {
    record.status = TaskStatus::Failed;
  }
  mutations.insert(0, KvMutation::put(rec_key(&id), encode_record(&record)?));
  Ok((mutations, TaskOpResult::ok().into_response()))
}

fn apply_requeue(
  read: &mut StateRead<'_>,
  id: String,
) -> Result<(Vec<KvMutation>, Response), String> {
  let Some(mut record) = read_record(read, &id)? else {
    return Ok((
      Vec::new(),
      TaskOpResult::rejected("task not found").into_response(),
    ));
  };
  if !matches!(record.status, TaskStatus::Assigned | TaskStatus::Running) {
    return Ok((
      Vec::new(),
      TaskOpResult::rejected(format!(
        "task is {}, not assigned/running",
        record.status.as_str()
      ))
      .into_response(),
    ));
  }

  let mut mutations = Vec::new();
  if let Some(node) = record.assigned_node_id.take() {
    mutations.push(KvMutation::del(assigned_idx_key(&node, &id)));
  }
  record.status = TaskStatus::Queued;
  record.lease_epoch = None;
  mutations.push(KvMutation::put(
    queued_idx_key(record.run_at, &id),
    id.clone(),
  ));
  mutations.insert(0, KvMutation::put(rec_key(&id), encode_record(&record)?));
  Ok((mutations, TaskOpResult::ok().into_response()))
}

fn apply_worker_lease(
  node_id: String,
  worker_name: String,
  lease_epoch: u64,
  expires_at: u64,
) -> Result<(Vec<KvMutation>, Response), String> {
  let record = WorkerLeaseRecord {
    node_id: node_id.clone(),
    worker_name,
    lease_epoch,
    expires_at,
  };
  let value = sonic_rs::to_string(&record).map_err(|err| format!("encode worker lease: {err}"))?;
  let mutations = vec![KvMutation::put(worker_key(&node_id), value)];
  Ok((mutations, TaskOpResult::ok().into_response()))
}

#[cfg(test)]
mod tests {
  use std::collections::BTreeMap;

  use super::*;

  struct MapState(BTreeMap<String, String>);

  impl MapState {
    fn new() -> Self {
      Self(BTreeMap::new())
    }

    fn apply(&mut self, cmd: Request) -> TaskOpResult {
      let map = self.0.clone();
      let mut read = move |key: &str| Ok(map.get(key).cloned());
      let (mutations, response) = apply_task_command(&mut read, cmd).expect("apply");
      for m in mutations {
        match m.value {
          Some(v) => {
            self.0.insert(m.key, v);
          }
          None => {
            self.0.remove(&m.key);
          }
        }
      }
      TaskOpResult::from_response(&response).expect("task op result")
    }

    fn record(&self, id: &str) -> TaskRecord {
      sonic_rs::from_str(self.0.get(&rec_key(id)).expect("record")).expect("decode")
    }

    fn has_key(&self, key: &str) -> bool {
      self.0.contains_key(key)
    }
  }

  fn enqueue(id: &str, idem: Option<&str>) -> Request {
    Request::TaskEnqueue {
      id: id.to_string(),
      payload: "{\"to\":\"a@b\"}".to_string(),
      run_at: 100,
      idem_key: idem.map(str::to_string),
    }
  }

  #[test]
  fn enqueue_creates_record_and_queued_index() {
    let mut state = MapState::new();
    let result = state.apply(enqueue("t1", Some("k1")));
    assert!(result.ok);
    assert_eq!(result.deduplicated, Some(false));
    assert_eq!(state.record("t1").status, TaskStatus::Queued);
    assert!(state.has_key(&queued_idx_key(100, "t1")));
    assert!(state.has_key(&idem_record_key("k1")));
  }

  #[test]
  fn enqueue_deduplicates_on_idem_key() {
    let mut state = MapState::new();
    state.apply(enqueue("t1", Some("k1")));
    let result = state.apply(enqueue("t2", Some("k1")));
    assert!(result.ok);
    assert_eq!(result.deduplicated, Some(true));
    assert_eq!(result.id.as_deref(), Some("t1"));
    assert!(!state.has_key(&rec_key("t2")));
  }

  #[test]
  fn assign_claim_done_lifecycle_maintains_indexes() {
    let mut state = MapState::new();
    state.apply(enqueue("t1", None));

    let result = state.apply(Request::TaskAssign {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 7,
    });
    assert!(result.ok);
    assert!(!state.has_key(&queued_idx_key(100, "t1")));
    assert!(state.has_key(&assigned_idx_key("nodeA", "t1")));
    assert_eq!(state.record("t1").status, TaskStatus::Assigned);

    // Claim by the wrong node is rejected atomically.
    let bad = state.apply(Request::TaskClaim {
      id: "t1".into(),
      node_id: "nodeB".into(),
      lease_epoch: 7,
    });
    assert!(!bad.ok);

    let claim = state.apply(Request::TaskClaim {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 7,
    });
    assert!(claim.ok);
    let claimed = claim.record.expect("claimed record");
    assert_eq!(claimed.attempts, 1);
    assert_eq!(state.record("t1").status, TaskStatus::Running);

    let done = state.apply(Request::TaskDone {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 7,
      attempts: 1,
    });
    assert!(done.ok);
    assert_eq!(state.record("t1").status, TaskStatus::Done);
    assert!(!state.has_key(&assigned_idx_key("nodeA", "t1")));
  }

  #[test]
  fn fail_with_retry_requeues_with_new_run_at() {
    let mut state = MapState::new();
    state.apply(enqueue("t1", None));
    state.apply(Request::TaskAssign {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
    });
    state.apply(Request::TaskClaim {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
    });

    let failed = state.apply(Request::TaskFail {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      attempts: 1,
      error: "boom".into(),
      retry_at: 200,
    });
    assert!(failed.ok);
    let record = state.record("t1");
    assert_eq!(record.status, TaskStatus::Queued);
    assert_eq!(record.run_at, 200);
    assert!(state.has_key(&queued_idx_key(200, "t1")));
    assert!(!state.has_key(&assigned_idx_key("nodeA", "t1")));

    // Permanent failure path.
    state.apply(Request::TaskAssign {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 2,
    });
    state.apply(Request::TaskClaim {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 2,
    });
    let dead = state.apply(Request::TaskFail {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 2,
      attempts: 2,
      error: "boom".into(),
      retry_at: 0,
    });
    assert!(dead.ok);
    assert_eq!(state.record("t1").status, TaskStatus::Failed);
  }

  #[test]
  fn requeue_returns_assigned_task_to_queue() {
    let mut state = MapState::new();
    state.apply(enqueue("t1", None));
    state.apply(Request::TaskAssign {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
    });

    let requeued = state.apply(Request::TaskRequeue { id: "t1".into() });
    assert!(requeued.ok);
    let record = state.record("t1");
    assert_eq!(record.status, TaskStatus::Queued);
    assert!(record.assigned_node_id.is_none());
    assert!(state.has_key(&queued_idx_key(100, "t1")));
    assert!(!state.has_key(&assigned_idx_key("nodeA", "t1")));

    // Stale ack from the pre-requeue assignment is rejected.
    let stale = state.apply(Request::TaskDone {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      attempts: 1,
    });
    assert!(!stale.ok);
  }

  #[test]
  fn queued_index_keys_sort_by_run_at() {
    let early = queued_idx_key(5, "a");
    let late = queued_idx_key(4_000_000_000, "b");
    assert!(early < late);
    assert_eq!(parse_queued_idx_key(&early), Some((5, "a")));
    assert_eq!(
      parse_assigned_idx_key(&assigned_idx_key("nodeA", "t1")),
      Some(("nodeA", "t1"))
    );
  }
}

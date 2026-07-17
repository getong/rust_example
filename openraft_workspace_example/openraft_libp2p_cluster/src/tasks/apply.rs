//! Deterministic state transitions of the task domain — the code the raft
//! state machine runs for every committed task command.
//!
//! `apply_task_command` performs every transition ATOMICALLY: the task
//! record, its secondary indexes (queued / assigned / terminal), the
//! idempotency table, and worker leases all change in the same write batch.
//!
//! Two invariants:
//!   - Determinism: this module never reads the clock or randomness; all timestamps come from the
//!     proposed command.
//!   - No execution in apply(): tasks are DATA here. Side effects run on exactly one worker via the
//!     claim/lease protocol (see [`crate::tasks::worker`]).

use super::{
  keys::{
    assigned_idx_key, idem_record_key, queued_idx_key, rec_key, terminal_idx_key, worker_key,
  },
  records::{TaskOpResult, TaskRecord, TaskStatus, WorkerLeaseRecord},
};
use crate::types_kv::{Response, TaskRequest};

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

/// A read of the CURRENT state-machine value for a key. Injected so the
/// logic stays storage-agnostic (RocksDB in production, a map in tests).
pub type StateRead<'a> = dyn FnMut(&str) -> Result<Option<String>, String> + 'a;

/// Returns `true` when `cmd` is a task command this module owns.
pub fn is_task_command(cmd: &crate::types_kv::Request) -> bool {
  matches!(cmd, crate::types_kv::Request::Task(_))
}

/// Commands whose apply may make a task schedulable; the state machine
/// notifies the local scheduler event channel after applying one of these,
/// so an idle leader reacts immediately instead of waiting for a tick.
pub fn is_schedule_event(cmd: &TaskRequest) -> bool {
  matches!(
    cmd,
    TaskRequest::TaskEnqueue { .. }
      | TaskRequest::TaskRequeue { .. }
      | TaskRequest::TaskReplay { .. }
      | TaskRequest::TaskFail { .. }
      | TaskRequest::WorkerLease { .. }
  )
}

/// Deterministically apply one task command against the current state.
/// Returns the key mutations (applied atomically by the caller in the same
/// write batch) and the proposer-visible response.
pub fn apply_task_command(
  read: &mut StateRead<'_>,
  cmd: TaskRequest,
) -> Result<(Vec<KvMutation>, Response), String> {
  match cmd {
    TaskRequest::TaskEnqueue {
      id,
      payload,
      run_at,
      idem_key,
      created_at,
    } => apply_enqueue(read, id, payload, run_at, idem_key, created_at),
    TaskRequest::TaskAssign {
      id,
      node_id,
      lease_epoch,
      now,
    } => apply_assign(read, id, node_id, lease_epoch, now),
    TaskRequest::TaskClaim {
      id,
      node_id,
      lease_epoch,
      now,
    } => apply_claim(read, id, node_id, lease_epoch, now),
    TaskRequest::TaskMarkCommitted {
      id,
      node_id,
      lease_epoch,
      now,
    } => apply_mark_committed(read, id, node_id, lease_epoch, now),
    TaskRequest::TaskDone {
      id,
      node_id,
      lease_epoch,
      attempts,
      now,
      result,
    } => apply_done(read, id, node_id, lease_epoch, attempts, now, result),
    TaskRequest::TaskFail {
      id,
      node_id,
      lease_epoch,
      attempts,
      error,
      retry_at,
      now,
    } => apply_fail(
      read,
      id,
      node_id,
      lease_epoch,
      attempts,
      error,
      retry_at,
      now,
    ),
    TaskRequest::TaskRequeue { id } => apply_requeue(read, id),
    TaskRequest::TaskReplay { id, now } => apply_replay(read, id, now),
    TaskRequest::TaskVacuum { ids } => apply_vacuum(read, ids),
    TaskRequest::WorkerLease {
      node_id,
      worker_name,
      lease_epoch,
      expires_at,
    } => apply_worker_lease(node_id, worker_name, lease_epoch, expires_at),
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
  created_at: u64,
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
    committed: false,
    error: None,
    updated_at: run_at,
    created_at,
    completed_at: 0,
    result: None,
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
  now: u64,
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
  record.updated_at = now;

  let mutations = vec![
    KvMutation::put(rec_key(&id), encode_record(&record)?),
    KvMutation::del(queued_key),
    // Index value carries the lease epoch so list_assigned can hand the
    // worker everything it needs to claim without touching the record.
    KvMutation::put(assigned_idx_key(&node_id, &id), lease_epoch.to_string()),
  ];
  Ok((mutations, TaskOpResult::ok().into_response()))
}

fn apply_claim(
  read: &mut StateRead<'_>,
  id: String,
  node_id: String,
  lease_epoch: u64,
  now: u64,
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
  record.updated_at = now;
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

/// Mark a running task as past its point of no return (renegade's
/// `Running.committed` pattern). Fenced exactly like an ack: (node,
/// lease_epoch) must match the record, so a worker whose task was already
/// requeued/reassigned gets a reject and MUST abort the side effect —
/// winning this replicated write is what resolves the race against a
/// concurrent requeue. Idempotent for the fenced owner.
fn apply_mark_committed(
  read: &mut StateRead<'_>,
  id: String,
  node_id: String,
  lease_epoch: u64,
  now: u64,
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
      TaskOpResult::rejected(format!(
        "commit-mark mismatch: status={}, assigned={:?}, lease={:?}",
        record.status.as_str(),
        record.assigned_node_id,
        record.lease_epoch
      ))
      .into_response(),
    ));
  }

  if record.committed {
    // Idempotent re-mark by the same fenced owner.
    return Ok((Vec::new(), TaskOpResult::ok().into_response()));
  }

  record.committed = true;
  record.updated_at = now;
  let mutations = vec![KvMutation::put(rec_key(&id), encode_record(&record)?)];
  Ok((mutations, TaskOpResult::ok().into_response()))
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
  now: u64,
  result: Option<String>,
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
  record.result = result;
  record.updated_at = now;
  record.completed_at = now;

  let mutations = vec![
    KvMutation::put(rec_key(&id), encode_record(&record)?),
    KvMutation::del(assigned_key),
    KvMutation::put(terminal_idx_key(now, &id), id.clone()),
  ];
  Ok((mutations, TaskOpResult::ok().into_response()))
}

#[allow(clippy::too_many_arguments)]
fn apply_fail(
  read: &mut StateRead<'_>,
  id: String,
  node_id: String,
  lease_epoch: u64,
  attempts: u32,
  error: String,
  retry_at: u64,
  now: u64,
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

  // A committed task is never blindly retried: a generic Err — notably the
  // execution timeout — does not prove the side effect did NOT run (it may
  // still be in flight). Force the terminal branch; the appended note makes
  // the suppression visible for reconciliation. Drills and clean pre-commit
  // failures are unaffected: handlers mark the commit point only right
  // before the side effect, so an Err raised earlier retries as always.
  let mut error = error;
  let mut retry_at = retry_at;
  if record.committed && retry_at > 0 {
    error.push_str(
      " (retry suppressed: task passed its commit point; side effect may have executed — needs \
       reconciliation)",
    );
    retry_at = 0;
  }

  let assigned_key = assigned_idx_key(&node_id, &id);
  record.attempts = attempts;
  record.error = Some(error);
  record.assigned_node_id = None;
  record.lease_epoch = None;
  record.updated_at = now;

  let mut mutations = vec![KvMutation::del(assigned_key)];
  if retry_at > 0 {
    // Delayed retry: back to the queue with the new due time.
    record.status = TaskStatus::Queued;
    record.run_at = retry_at;
    mutations.push(KvMutation::put(queued_idx_key(retry_at, &id), id.clone()));
  } else {
    record.status = TaskStatus::Failed;
    record.completed_at = now;
    mutations.push(KvMutation::put(terminal_idx_key(now, &id), id.clone()));
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
  record.lease_epoch = None;

  if record.committed {
    // The task declared its point of no return: the side effect may already
    // have executed, so a re-run could duplicate it (renegade's rule for
    // orphaned committed settles: never re-run, surface for reconciliation).
    // Fail it terminally instead of re-queueing. Deciding this here, in
    // apply, keeps the policy deterministic on every replica no matter which
    // scheduler path (dead worker / stuck / disconnect grace) proposed the
    // requeue.
    let now = record.updated_at.max(record.created_at);
    record.status = TaskStatus::Failed;
    record.completed_at = now;
    record.error = Some(
      "requeue refused: task passed its commit point on a lost worker; side effect may have \
       executed — needs reconciliation"
        .to_string(),
    );
    mutations.push(KvMutation::put(terminal_idx_key(now, &id), id.clone()));
    mutations.insert(0, KvMutation::put(rec_key(&id), encode_record(&record)?));
    let result = TaskOpResult {
      ok: true,
      id: Some(id),
      deduplicated: None,
      record: None,
      reason: Some("committed task failed terminally instead of requeued".to_string()),
    };
    return Ok((mutations, result.into_response()));
  }

  record.status = TaskStatus::Queued;
  mutations.push(KvMutation::put(
    queued_idx_key(record.run_at, &id),
    id.clone(),
  ));
  mutations.insert(0, KvMutation::put(rec_key(&id), encode_record(&record)?));
  Ok((mutations, TaskOpResult::ok().into_response()))
}

/// Operator-driven dead-letter replay: Failed → Queued with a fresh attempt
/// budget and `run_at = now`. Committed tasks are refused — their side
/// effect may already have executed, so a replay could duplicate it; they
/// need reconciliation, not a re-run.
fn apply_replay(
  read: &mut StateRead<'_>,
  id: String,
  now: u64,
) -> Result<(Vec<KvMutation>, Response), String> {
  let Some(mut record) = read_record(read, &id)? else {
    return Ok((
      Vec::new(),
      TaskOpResult::rejected("task not found").into_response(),
    ));
  };
  if record.status != TaskStatus::Failed {
    return Ok((
      Vec::new(),
      TaskOpResult::rejected(format!("task is {}, not failed", record.status.as_str()))
        .into_response(),
    ));
  }
  if record.committed {
    return Ok((
      Vec::new(),
      TaskOpResult::rejected(
        "replay refused: task passed its commit point; side effect may have executed — needs \
         reconciliation",
      )
      .into_response(),
    ));
  }

  let terminal_key = terminal_idx_key(record.completed_at, &id);
  record.status = TaskStatus::Queued;
  record.attempts = 0;
  record.run_at = now;
  record.error = None;
  record.updated_at = now;
  record.completed_at = 0;

  let mutations = vec![
    KvMutation::put(rec_key(&id), encode_record(&record)?),
    KvMutation::del(terminal_key),
    KvMutation::put(queued_idx_key(now, &id), id.clone()),
  ];
  let result = TaskOpResult {
    ok: true,
    id: Some(id),
    deduplicated: None,
    record: None,
    reason: None,
  };
  Ok((mutations, result.into_response()))
}

/// Delete terminal (done/failed) records plus their terminal-index entries
/// and idempotency keys. Non-terminal or missing ids are skipped, so a
/// vacuum proposed from a slightly stale scan stays safe and deterministic.
fn apply_vacuum(
  read: &mut StateRead<'_>,
  ids: Vec<String>,
) -> Result<(Vec<KvMutation>, Response), String> {
  let mut mutations = Vec::new();
  let mut removed = 0usize;

  for id in ids {
    let Some(record) = read_record(read, &id)? else {
      continue;
    };
    if !matches!(record.status, TaskStatus::Done | TaskStatus::Failed) {
      continue;
    }
    mutations.push(KvMutation::del(rec_key(&id)));
    mutations.push(KvMutation::del(terminal_idx_key(record.completed_at, &id)));
    if let Some(idem) = record.idem_key.as_deref() {
      mutations.push(KvMutation::del(idem_record_key(idem)));
    }
    removed += 1;
  }

  let result = TaskOpResult {
    ok: true,
    id: None,
    deduplicated: None,
    record: None,
    reason: Some(format!("vacuumed {removed}")),
  };
  Ok((mutations, result.into_response()))
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
  use crate::tasks::{
    keys::{parse_assigned_idx_key, parse_queued_idx_key},
    records::compute_metrics,
  };

  struct MapState(BTreeMap<String, String>);

  impl MapState {
    fn new() -> Self {
      Self(BTreeMap::new())
    }

    fn apply(&mut self, cmd: TaskRequest) -> TaskOpResult {
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

  fn enqueue(id: &str, idem: Option<&str>) -> TaskRequest {
    TaskRequest::TaskEnqueue {
      id: id.to_string(),
      payload: "{\"to\":\"a@b\"}".to_string(),
      run_at: 100,
      idem_key: idem.map(str::to_string),
      created_at: 100,
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

    let result = state.apply(TaskRequest::TaskAssign {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 7,
      now: 1000,
    });
    assert!(result.ok);
    assert!(!state.has_key(&queued_idx_key(100, "t1")));
    // Index value must carry the lease epoch (list_assigned relies on it).
    assert_eq!(
      state
        .0
        .get(&assigned_idx_key("nodeA", "t1"))
        .map(String::as_str),
      Some("7")
    );
    assert_eq!(state.record("t1").status, TaskStatus::Assigned);

    // Claim by the wrong node is rejected atomically.
    let bad = state.apply(TaskRequest::TaskClaim {
      id: "t1".into(),
      node_id: "nodeB".into(),
      lease_epoch: 7,
      now: 1001,
    });
    assert!(!bad.ok);

    let claim = state.apply(TaskRequest::TaskClaim {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 7,
      now: 1001,
    });
    assert!(claim.ok);
    let claimed = claim.record.expect("claimed record");
    assert_eq!(claimed.attempts, 1);
    assert_eq!(state.record("t1").status, TaskStatus::Running);

    let done = state.apply(TaskRequest::TaskDone {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 7,
      attempts: 1,
      now: 1002,
      result: Some("{\"delivered\":true}".to_string()),
    });
    assert!(done.ok);
    let record = state.record("t1");
    assert_eq!(record.status, TaskStatus::Done);
    assert_eq!(record.result.as_deref(), Some("{\"delivered\":true}"));
    assert_eq!(record.created_at, 100);
    assert_eq!(record.completed_at, 1002);
    assert!(!state.has_key(&assigned_idx_key("nodeA", "t1")));
    assert!(state.has_key(&terminal_idx_key(1002, "t1")));
  }

  #[test]
  fn fail_with_retry_requeues_with_new_run_at() {
    let mut state = MapState::new();
    state.apply(enqueue("t1", None));
    state.apply(TaskRequest::TaskAssign {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      now: 1000,
    });
    state.apply(TaskRequest::TaskClaim {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      now: 1001,
    });

    let failed = state.apply(TaskRequest::TaskFail {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      attempts: 1,
      error: "boom".into(),
      retry_at: 200,
      now: 1002,
    });
    assert!(failed.ok);
    let record = state.record("t1");
    assert_eq!(record.status, TaskStatus::Queued);
    assert_eq!(record.run_at, 200);
    assert!(state.has_key(&queued_idx_key(200, "t1")));
    assert!(!state.has_key(&assigned_idx_key("nodeA", "t1")));

    // Permanent failure path.
    state.apply(TaskRequest::TaskAssign {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 2,
      now: 1000,
    });
    state.apply(TaskRequest::TaskClaim {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 2,
      now: 1001,
    });
    let dead = state.apply(TaskRequest::TaskFail {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 2,
      attempts: 2,
      error: "boom".into(),
      retry_at: 0,
      now: 1003,
    });
    assert!(dead.ok);
    let record = state.record("t1");
    assert_eq!(record.status, TaskStatus::Failed);
    assert_eq!(record.completed_at, 1003);
    assert!(state.has_key(&terminal_idx_key(1003, "t1")));
  }

  #[test]
  fn requeue_returns_assigned_task_to_queue() {
    let mut state = MapState::new();
    state.apply(enqueue("t1", None));
    state.apply(TaskRequest::TaskAssign {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      now: 1000,
    });

    let requeued = state.apply(TaskRequest::TaskRequeue { id: "t1".into() });
    assert!(requeued.ok);
    let record = state.record("t1");
    assert_eq!(record.status, TaskStatus::Queued);
    assert!(record.assigned_node_id.is_none());
    assert!(state.has_key(&queued_idx_key(100, "t1")));
    assert!(!state.has_key(&assigned_idx_key("nodeA", "t1")));

    // Stale ack from the pre-requeue assignment is rejected.
    let stale = state.apply(TaskRequest::TaskDone {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      attempts: 1,
      now: 1002,
      result: None,
    });
    assert!(!stale.ok);
  }

  /// Helper: enqueue → assign → claim, leaving `id` Running on (nodeA, 1).
  fn running_task(state: &mut MapState, id: &str) {
    state.apply(enqueue(id, None));
    state.apply(TaskRequest::TaskAssign {
      id: id.into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      now: 1000,
    });
    state.apply(TaskRequest::TaskClaim {
      id: id.into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      now: 1001,
    });
  }

  #[test]
  fn mark_committed_is_fenced_and_idempotent() {
    let mut state = MapState::new();
    running_task(&mut state, "t1");

    // A stale (node, epoch) pair cannot mark the commit point.
    let stale = state.apply(TaskRequest::TaskMarkCommitted {
      id: "t1".into(),
      node_id: "nodeB".into(),
      lease_epoch: 9,
      now: 1002,
    });
    assert!(!stale.ok);
    assert!(!state.record("t1").committed);

    // The fenced owner can; a re-mark is an idempotent ok.
    for _ in 0 .. 2 {
      let marked = state.apply(TaskRequest::TaskMarkCommitted {
        id: "t1".into(),
        node_id: "nodeA".into(),
        lease_epoch: 1,
        now: 1002,
      });
      assert!(marked.ok);
      assert!(state.record("t1").committed);
    }
  }

  #[test]
  fn requeue_of_committed_task_fails_terminally() {
    let mut state = MapState::new();
    running_task(&mut state, "t1");
    state.apply(TaskRequest::TaskMarkCommitted {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      now: 1002,
    });

    // A scheduler requeue (dead/stuck worker) must NOT re-run the task.
    let requeued = state.apply(TaskRequest::TaskRequeue { id: "t1".into() });
    assert!(requeued.ok);
    let record = state.record("t1");
    assert_eq!(record.status, TaskStatus::Failed);
    assert!(record.completed_at > 0);
    assert!(
      record
        .error
        .as_deref()
        .unwrap_or("")
        .contains("commit point")
    );
    assert!(!state.has_key(&queued_idx_key(100, "t1")));
    assert!(!state.has_key(&assigned_idx_key("nodeA", "t1")));
    assert!(state.has_key(&terminal_idx_key(record.completed_at, "t1")));
  }

  #[test]
  fn fail_of_committed_task_suppresses_retry() {
    let mut state = MapState::new();
    running_task(&mut state, "t1");
    state.apply(TaskRequest::TaskMarkCommitted {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      now: 1002,
    });

    // A generic failure with a retry slot (e.g. execution timeout) lands
    // terminal instead of re-queueing: the side effect may have executed.
    let failed = state.apply(TaskRequest::TaskFail {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      attempts: 1,
      error: "execution timed out".into(),
      retry_at: 2000,
      now: 1030,
    });
    assert!(failed.ok);
    let record = state.record("t1");
    assert_eq!(record.status, TaskStatus::Failed);
    assert!(
      record
        .error
        .as_deref()
        .unwrap_or("")
        .contains("retry suppressed")
    );
    assert!(!state.has_key(&queued_idx_key(2000, "t1")));
    assert!(state.has_key(&terminal_idx_key(1030, "t1")));
  }

  /// Helper: drive `id` to permanent failure on (nodeA, 1) at now=1003.
  fn failed_task(state: &mut MapState, id: &str) {
    running_task(state, id);
    state.apply(TaskRequest::TaskFail {
      id: id.into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      attempts: 3,
      error: "boom".into(),
      retry_at: 0,
      now: 1003,
    });
  }

  #[test]
  fn replay_returns_failed_task_to_queue() {
    let mut state = MapState::new();
    failed_task(&mut state, "t1");
    assert_eq!(state.record("t1").status, TaskStatus::Failed);

    let replayed = state.apply(TaskRequest::TaskReplay {
      id: "t1".into(),
      now: 2000,
    });
    assert!(replayed.ok);
    let record = state.record("t1");
    assert_eq!(record.status, TaskStatus::Queued);
    assert_eq!(record.attempts, 0);
    assert_eq!(record.run_at, 2000);
    assert_eq!(record.updated_at, 2000);
    assert_eq!(record.completed_at, 0);
    assert!(record.error.is_none());
    assert!(state.has_key(&queued_idx_key(2000, "t1")));
    assert!(!state.has_key(&terminal_idx_key(1003, "t1")));

    // The replayed task is invisible to vacuum until it fails again.
    let vacuumed = state.apply(TaskRequest::TaskVacuum {
      ids: vec!["t1".into()],
    });
    assert_eq!(vacuumed.reason.as_deref(), Some("vacuumed 0"));
    assert!(state.has_key(&rec_key("t1")));
  }

  #[test]
  fn replay_rejects_non_failed_task() {
    let mut state = MapState::new();
    state.apply(enqueue("t1", None));

    let replayed = state.apply(TaskRequest::TaskReplay {
      id: "t1".into(),
      now: 2000,
    });
    assert!(!replayed.ok);
    assert_eq!(state.record("t1").status, TaskStatus::Queued);
    // Original due time untouched; no duplicate queued-index entry created.
    assert!(state.has_key(&queued_idx_key(100, "t1")));
    assert!(!state.has_key(&queued_idx_key(2000, "t1")));

    let missing = state.apply(TaskRequest::TaskReplay {
      id: "nope".into(),
      now: 2000,
    });
    assert!(!missing.ok);
  }

  #[test]
  fn replay_refuses_committed_task() {
    let mut state = MapState::new();
    running_task(&mut state, "t1");
    state.apply(TaskRequest::TaskMarkCommitted {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      now: 1002,
    });
    // Timeout after the commit point lands terminal with committed=true.
    state.apply(TaskRequest::TaskFail {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      attempts: 1,
      error: "execution timed out".into(),
      retry_at: 2000,
      now: 1030,
    });
    let record = state.record("t1");
    assert_eq!(record.status, TaskStatus::Failed);
    assert!(record.committed);

    let replayed = state.apply(TaskRequest::TaskReplay {
      id: "t1".into(),
      now: 3000,
    });
    assert!(!replayed.ok);
    assert!(
      replayed
        .reason
        .as_deref()
        .unwrap_or("")
        .contains("commit point")
    );
    // Terminal state and index untouched.
    assert_eq!(state.record("t1").status, TaskStatus::Failed);
    assert!(state.has_key(&terminal_idx_key(1030, "t1")));
    assert!(!state.has_key(&queued_idx_key(3000, "t1")));
  }

  #[test]
  fn vacuum_deletes_only_terminal_records_and_idem_keys() {
    let mut state = MapState::new();
    // t1: done (with idem key). t2: still queued. t3: permanently failed.
    state.apply(enqueue("t1", Some("k1")));
    state.apply(enqueue("t2", None));
    state.apply(enqueue("t3", None));
    for (id, epoch) in [("t1", 1u64), ("t3", 2u64)] {
      state.apply(TaskRequest::TaskAssign {
        id: id.into(),
        node_id: "nodeA".into(),
        lease_epoch: epoch,
        now: 1000,
      });
      state.apply(TaskRequest::TaskClaim {
        id: id.into(),
        node_id: "nodeA".into(),
        lease_epoch: epoch,
        now: 1001,
      });
    }
    state.apply(TaskRequest::TaskDone {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      attempts: 1,
      now: 1002,
      result: None,
    });
    state.apply(TaskRequest::TaskFail {
      id: "t3".into(),
      node_id: "nodeA".into(),
      lease_epoch: 2,
      attempts: 3,
      error: "boom".into(),
      retry_at: 0,
      now: 1003,
    });

    // Queued t2 is skipped even though it was (incorrectly) listed.
    let result = state.apply(TaskRequest::TaskVacuum {
      ids: vec!["t1".into(), "t2".into(), "t3".into(), "missing".into()],
    });
    assert!(result.ok);
    assert_eq!(result.reason.as_deref(), Some("vacuumed 2"));

    assert!(!state.has_key(&rec_key("t1")));
    assert!(!state.has_key(&terminal_idx_key(1002, "t1")));
    assert!(!state.has_key(&idem_record_key("k1")));
    assert!(!state.has_key(&rec_key("t3")));
    assert!(!state.has_key(&terminal_idx_key(1003, "t3")));
    // Live task untouched.
    assert!(state.has_key(&rec_key("t2")));
    assert!(state.has_key(&queued_idx_key(100, "t2")));
  }

  #[test]
  fn compute_metrics_reports_completion_latency() {
    let done = TaskRecord {
      id: "t1".into(),
      payload: String::new(),
      status: TaskStatus::Done,
      attempts: 1,
      run_at: 100,
      idem_key: None,
      assigned_node_id: None,
      lease_epoch: None,
      committed: false,
      error: None,
      updated_at: 130,
      created_at: 100,
      completed_at: 130,
      result: None,
    };
    let failed = TaskRecord {
      id: "t2".into(),
      status: TaskStatus::Failed,
      completed_at: 110,
      ..done.clone()
    };
    // Pre-timestamp record: excluded from latency aggregation.
    let legacy = TaskRecord {
      id: "t3".into(),
      created_at: 0,
      completed_at: 0,
      ..done.clone()
    };
    let metrics = compute_metrics(&[done, failed, legacy], &[], 200);
    assert_eq!(metrics.max_completion_latency_secs, 30);
    assert_eq!(metrics.avg_completion_latency_secs, 20); // (30 + 10) / 2
  }

  #[test]
  fn compute_metrics_aggregates_statuses_and_workers() {
    let mut state = MapState::new();
    state.apply(enqueue("t1", None)); // stays queued (run_at=100)
    state.apply(enqueue("t2", None));
    state.apply(TaskRequest::TaskAssign {
      id: "t2".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      now: 1000,
    });
    state.apply(TaskRequest::TaskClaim {
      id: "t2".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      now: 1001,
    });

    let records: Vec<TaskRecord> = vec![state.record("t1"), state.record("t2")];
    let leases = vec![
      WorkerLeaseRecord {
        node_id: "nodeA".into(),
        worker_name: "a".into(),
        lease_epoch: 1,
        expires_at: 2000, // active at now=1500
      },
      WorkerLeaseRecord {
        node_id: "nodeB".into(),
        worker_name: "b".into(),
        lease_epoch: 1,
        expires_at: 100, // expired
      },
    ];

    let metrics = compute_metrics(&records, &leases, 1500);
    assert_eq!(metrics.total, 2);
    assert_eq!(metrics.queued, 1);
    assert_eq!(metrics.running, 1);
    assert_eq!(metrics.total_attempts, 1);
    assert_eq!(metrics.retried_tasks, 0);
    assert_eq!(metrics.oldest_due_queued_age_secs, 1400); // 1500 - run_at(100)
    assert_eq!(metrics.active_workers, 1);
    assert_eq!(metrics.total_worker_leases, 2);
  }

  #[test]
  fn assign_and_claim_stamp_updated_at() {
    let mut state = MapState::new();
    state.apply(enqueue("t1", None));
    state.apply(TaskRequest::TaskAssign {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      now: 5000,
    });
    assert_eq!(state.record("t1").updated_at, 5000);
    state.apply(TaskRequest::TaskClaim {
      id: "t1".into(),
      node_id: "nodeA".into(),
      lease_epoch: 1,
      now: 5010,
    });
    assert_eq!(state.record("t1").updated_at, 5010);
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

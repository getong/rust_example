//! Key-space layout of the task domain inside the replicated KV state.
//!
//! Every piece of task state lives under a reserved `task:` prefix; the
//! sub-prefixes below partition it into the primary record space, three
//! secondary indexes, the idempotency table, and the worker lease table.
//! All builders/parsers for those keys live here so the encoding cannot
//! drift between the state machine, the scheduler and the RPC readers.

pub const TASK_REC_PREFIX: &str = "task:rec:";
pub const TASK_QUEUED_IDX_PREFIX: &str = "task:idx:queued:";
pub const TASK_ASSIGNED_IDX_PREFIX: &str = "task:idx:assigned:";
/// Terminal (done/failed) tasks sorted by completion time; the leader's
/// vacuum pass scans this instead of the full record space.
pub const TASK_TERMINAL_IDX_PREFIX: &str = "task:idx:terminal:";
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

/// Zero-padded completion time keeps the terminal index sorted, so the
/// vacuum scan stops at the retention cutoff.
pub fn terminal_idx_key(completed_at: u64, id: &str) -> String {
  format!("{TASK_TERMINAL_IDX_PREFIX}{completed_at:020}:{id}")
}

/// Parse `task:idx:terminal:{completed_at:020}:{id}` → (completed_at, id).
pub fn parse_terminal_idx_key(key: &str) -> Option<(u64, &str)> {
  let rest = key.strip_prefix(TASK_TERMINAL_IDX_PREFIX)?;
  let (completed_at, id) = rest.split_once(':')?;
  Some((completed_at.parse().ok()?, id))
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

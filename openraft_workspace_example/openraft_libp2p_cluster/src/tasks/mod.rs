//! Task-native replicated task management (octopii-style).
//!
//! A task is a domain command in the raft log: OpenRaft does not schedule
//! threads or executors — it decides the globally consistent ORDER of
//! business commands (who enqueued first, who claimed first, how state
//! changes). Everything else is layered around that ordering:
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │ 接入层  api::TaskApi — ONE facade for every frontend           │
//! │   enqueue / replay / submit / list / metrics                   │
//! │   (control node → local raft; worker → TaskRpc + leader hint)  │
//! ├────────────────────────────────────────────────────────────────┤
//! │ 传输层  rpc — tarpc TaskRpc over libp2p /openraft/task/1       │
//! ├────────────────────────────────────────────────────────────────┤
//! │ 命令层  types_kv::TaskRequest — replicated domain commands     │
//! │   Enqueue / Assign / Claim / MarkCommitted / Done / Fail /     │
//! │   Requeue / Replay / Vacuum / WorkerLease                      │
//! ├────────────────────────────────────────────────────────────────┤
//! │ 状态机层  apply — deterministic transitions (raft apply step)  │
//! │   record + queued/assigned/terminal indexes + idempotency      │
//! │   table + leases, all in ONE atomic write batch                │
//! │   keys — key-space layout   records — data model + metrics     │
//! ├────────────────────────────────────────────────────────────────┤
//! │ 调度层  scheduler — leader-only, event-driven                  │
//! │   assign due tasks, requeue dead/stuck work, vacuum retention  │
//! ├────────────────────────────────────────────────────────────────┤
//! │ 执行层  worker + handlers — the layer raft does NOT provide    │
//! │   claim → execute side effect on exactly ONE node → ack;       │
//! │   handlers dispatch by payload kind (TaskPayload enum)         │
//! └────────────────────────────────────────────────────────────────┘
//! ```
//!
//! Two invariants hold everywhere:
//!   - Determinism: [`apply`] never reads the clock or randomness; every timestamp is
//!     proposer-supplied inside the command.
//!   - No execution in apply(): tasks are DATA in the state machine. Side effects run on exactly
//!     one worker via the claim/lease fencing protocol; irreversible effects additionally use the
//!     `TaskMarkCommitted` commit-point protocol so they are never duplicated by a retry.

pub mod api;
pub mod apply;
pub mod events;
pub mod handlers;
pub mod keys;
pub mod records;
pub mod rpc;
pub mod scheduler;
pub mod worker;

// The submodule split is an internal layering; `crate::tasks::X` stays the
// public path for all of it.
pub use apply::{KvMutation, StateRead, apply_task_command, is_schedule_event, is_task_command};
pub use keys::{
  TASK_ASSIGNED_IDX_PREFIX, TASK_IDEM_PREFIX, TASK_QUEUED_IDX_PREFIX, TASK_REC_PREFIX,
  TASK_TERMINAL_IDX_PREFIX, TASK_WORKER_PREFIX, assigned_idx_key, assigned_idx_node_prefix,
  idem_record_key, parse_assigned_idx_key, parse_queued_idx_key, parse_terminal_idx_key,
  queued_idx_key, rec_key, terminal_idx_key, worker_key,
};
pub use records::{
  TaskOpResult, TaskQueueMetrics, TaskRecord, TaskStatus, WorkerLeaseRecord, compute_metrics,
};

/// Maximum executions per task before it is marked failed permanently.
pub const MAX_TASK_ATTEMPTS: u32 = 3;

/// Hard cap on one stored task payload, enforced at every enqueue door
/// (HTTP and task RPC). Wasm tasks carry the handler MODULE inside the
/// payload, and every payload byte is replicated through the raft log and
/// into snapshots on all nodes — oversized modules must be rejected up
/// front rather than degrade the whole log.
pub const MAX_TASK_PAYLOAD_BYTES: usize = 256 * 1024;

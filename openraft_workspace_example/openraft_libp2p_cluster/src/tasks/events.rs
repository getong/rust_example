//! In-process schedule events derived from the state machine.
//!
//! The raft `apply()` step is the single serialization point for every task
//! transition, so it doubles as a change feed: after a batch containing a
//! schedule-relevant command (enqueue / requeue / retry-fail / worker lease)
//! is durably applied, the state machine bumps this watch channel. The
//! leader's scheduler subscribes and reacts immediately; on followers the
//! bump is a no-op. This replaces tight fixed-interval polling — the
//! periodic tick remains only as a slow fallback and for time-based work
//! (due run_at, stuck detection, lease expiry).

use std::sync::OnceLock;

use tokio::sync::watch;

static SCHEDULE_EVENTS: OnceLock<watch::Sender<u64>> = OnceLock::new();

fn channel() -> &'static watch::Sender<u64> {
  SCHEDULE_EVENTS.get_or_init(|| watch::channel(0).0)
}

/// Called by the state machine after applying schedule-relevant commands.
/// Never blocks; safe to call from apply.
pub fn notify_schedule() {
  channel().send_modify(|counter| *counter = counter.wrapping_add(1));
}

/// Subscribe to schedule events (used by the leader's scheduler).
pub fn subscribe() -> watch::Receiver<u64> {
  channel().subscribe()
}

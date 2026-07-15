//! Self-healing membership: each raft group's leader watches every member
//! (voters AND learners). When a member stays unreachable past a timeout:
//!   - a dead voter is replaced by a connected learner (one `change_membership` removes the dead
//!     voter and promotes the learner together, keeping the quorum math sound);
//!   - a dead learner is removed from the membership;
//! and in both cases the learner pool is backfilled from a spare connected
//! worker.
//!
//! "Unreachable" is judged from TWO observers, and a member is treated as
//! alive if EITHER says so:
//!   - the libp2p liveness view (`network.is_peer_alive`: a direct connection or a fresh
//!     node-announce gossip), and
//!   - openraft's own heartbeat acknowledgements (`RaftMetrics::heartbeat`, the leader-local
//!     last-ack time per member — a member that recently acked AppendEntries is alive at the raft
//!     layer no matter what the transport view says).
//! Only a member both observers consider dead starts the down clock. This
//! halves the false-replacement surface: a transport blip with healthy raft
//! replication, or a connection that libp2p reports while the raft runtime
//! is wedged, no longer triggers (or respectively no longer masks) a
//! replacement on its own.

use std::{
  collections::{BTreeSet, HashMap},
  sync::Arc,
  time::Duration,
};

use anyhow::anyhow;
use openraft::{BasicNode, ChangeMembers, Instant as _, async_runtime::WatchReceiver};
use tokio::time::{Instant, timeout};

use crate::{
  GroupId, NodeId,
  network::transport::Libp2pNetworkFactory,
  signal::ShutdownRx,
  typ::{Raft, RaftMetrics},
};

#[derive(Clone, Debug)]
pub struct MembershipGuardConfig {
  /// How often the guard inspects the membership. The unreachable-member
  /// timeout itself is hot-reloadable and read from
  /// [`crate::runtime_config`] on every tick.
  pub tick_interval: Duration,
}

/// Upper bound on a single membership change (`change_membership` /
/// `add_learner`). Around a marginal quorum these calls can block until the
/// change commits — potentially forever — which would wedge the guard loop
/// on the exact ticks where it is needed most. On timeout the tick fails and
/// the next tick re-evaluates from fresh metrics; openraft resolves a
/// re-proposed identical change idempotently.
const MEMBERSHIP_CHANGE_TIMEOUT: Duration = Duration::from_secs(30);

/// A member whose last raft heartbeat/replication ack is younger than this
/// counts as alive regardless of the libp2p connection view. Openraft sends
/// heartbeats every `Config::heartbeat_interval` (sub-second), so several
/// missed intervals fit comfortably inside this window; it is deliberately
/// far below any sane `voter_replace_timeout`, so raft-ack freshness delays
/// no replacement — it only vetoes false ones.
///
/// This is the BASE window: when the gossipsub mesh is degraded (partition
/// recovery, large cluster churn) the announce-based liveness observer is
/// unreliable, so the window is stretched up to
/// [`RAFT_ACK_TIMEOUT_MAX_SCALE`]x to lean harder on openraft's own signal
/// and avoid false replacements.
const RAFT_ACK_FRESH_TIMEOUT: Duration = Duration::from_secs(10);

/// Multiplier applied to [`RAFT_ACK_FRESH_TIMEOUT`] when the gossipsub mesh
/// health is 0 (fully degraded). Health values in between scale linearly.
const RAFT_ACK_TIMEOUT_MAX_SCALE: f64 = 3.0;

/// Stretch the raft-ack freshness window as the gossipsub mesh degrades:
/// health 1.0 keeps the base window, health 0.0 triples it.
fn scaled_raft_ack_timeout(mesh_health: f64) -> Duration {
  let health = mesh_health.clamp(0.0, 1.0);
  let scale = 1.0 + (RAFT_ACK_TIMEOUT_MAX_SCALE - 1.0) * (1.0 - health);
  RAFT_ACK_FRESH_TIMEOUT.mul_f64(scale)
}

/// Current raft-ack freshness window, adjusted by live gossipsub mesh
/// health. Degrades to the base window when the health signal is
/// unavailable.
async fn raft_ack_fresh_timeout(group_id: &str, network: &Libp2pNetworkFactory) -> Duration {
  let Some(health) = network
    .gossipsub_mesh_health(crate::network::swarm::NODE_ANNOUNCE_TOPIC)
    .await
  else {
    return RAFT_ACK_FRESH_TIMEOUT;
  };
  let timeout = scaled_raft_ack_timeout(health);
  if timeout > RAFT_ACK_FRESH_TIMEOUT {
    tracing::debug!(
      group = %group_id,
      mesh_health = health,
      ack_timeout = ?timeout,
      "gossipsub mesh degraded; stretching raft-ack liveness window"
    );
  }
  timeout
}

pub async fn run_membership_guard(
  group_id: GroupId,
  raft: Raft,
  network: Libp2pNetworkFactory,
  config: MembershipGuardConfig,
  membership_fence: Arc<tokio::sync::Mutex<()>>,
  mut shutdown_rx: ShutdownRx,
) -> anyhow::Result<()> {
  let mut down_since: HashMap<NodeId, Instant> = HashMap::new();
  let mut tick = tokio::time::interval(config.tick_interval);
  tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!(group = %group_id, "stopping openraft membership guard");
        return Ok(());
      }
      _ = tick.tick() => {
        if let Err(err) =
          guard_tick(&group_id, &raft, &network, &mut down_since, &membership_fence).await
        {
          tracing::warn!(group = %group_id, error = ?err, "membership guard tick failed; retrying next tick");
        }
      }
    }
  }
}

async fn guard_tick(
  group_id: &str,
  raft: &Raft,
  network: &Libp2pNetworkFactory,
  down_since: &mut HashMap<NodeId, Instant>,
  membership_fence: &Arc<tokio::sync::Mutex<()>>,
) -> anyhow::Result<()> {
  // Liveness of the guard itself: a flat `membership_guard_tick_total` in a
  // dashboard means the guard task died or is wedged — the one failure mode
  // `membership_guard_replacement_total` (which is usually 0) cannot show.
  metrics::counter!(
    "membership_guard_tick_total",
    "group" => group_id.to_string(),
  )
  .increment(1);

  let metrics = raft.metrics().borrow_watched().clone();
  if !metrics.state.is_leader() {
    // The leader controller stops this task on role change, but clear the
    // tracking anyway so a stale map never survives a re-election.
    down_since.clear();
    return Ok(());
  }

  let membership = metrics.membership_config.membership();
  let voters: BTreeSet<NodeId> = membership.voter_ids().collect();
  let learners: Vec<NodeId> = membership.learner_ids().collect();
  let member_ids: BTreeSet<NodeId> = membership.nodes().map(|(id, _)| id.clone()).collect();
  let self_id = metrics.id.clone();

  // Second observer: members whose last raft heartbeat/replication ack is
  // fresh (leader-local view from openraft itself). Absent map (not leader /
  // no data yet) degrades to the libp2p-only view. The freshness window
  // stretches with gossipsub mesh degradation, since a degraded mesh makes
  // the announce-based observer unreliable.
  let ack_timeout = raft_ack_fresh_timeout(group_id, network).await;
  let raft_ack_fresh = fresh_raft_acks(&metrics, ack_timeout);

  // Track for how long each member (voter or learner) has been unreachable.
  // A member is alive if EITHER observer says so: a libp2p connection is
  // visible, or it acked raft traffic recently. Recovery on either signal
  // resets the clock, so transient drops within the timeout are ignored.
  let now = Instant::now();
  for member in &member_ids {
    if member == &self_id {
      continue;
    }
    if node_alive(network, member).await || raft_ack_fresh.contains(member) {
      down_since.remove(member);
    } else {
      down_since.entry(member.clone()).or_insert(now);
    }
    // Per-member downtime gauge: 0 while alive, the current down-clock while
    // not. Charts of it show how close each member gets to the replace
    // timeout, and for how long members flap below it.
    let downtime = down_since
      .get(member)
      .map(|since| now.duration_since(*since).as_secs_f64())
      .unwrap_or(0.0);
    metrics::gauge!(
      "membership_guard_member_downtime_seconds",
      "group" => group_id.to_string(),
      "member" => member.to_string(),
    )
    .set(downtime);
  }
  down_since.retain(|id, _| member_ids.contains(id) && id != &self_id);

  // Act on at most ONE member per tick; pick the longest-down one. The
  // timeout is hot-reloadable (POST /config), so it is read per tick.
  let replace_timeout = crate::runtime_config::current().voter_replace_timeout();
  let expired = down_since
    .iter()
    .map(|(id, since)| (id.clone(), now.duration_since(*since)))
    .filter(|(_, downtime)| *downtime >= replace_timeout)
    .max_by_key(|(_, downtime)| *downtime);

  let Some((dead_member, downtime)) = expired else {
    return Ok(());
  };

  // One membership change at a time — across the WHOLE process, not just
  // this guard: the fence is shared with the HTTP membership handlers and
  // the AddLearner/JoinCluster RPC paths (GroupHandle::membership_fence).
  // While any change is still committing, the tick only keeps tracking
  // liveness; the down clock is preserved, so a failed change is re-proposed
  // immediately on a later tick instead of waiting a full timeout again.
  let Ok(fence) = membership_fence.clone().try_lock_owned() else {
    tracing::debug!(
      group = %group_id,
      dead_member = %dead_member,
      "membership change already in flight; deferring action to a later tick"
    );
    return Ok(());
  };

  // Pick the action while the tick-local observer data is at hand; the
  // (potentially slow) raft write then runs off-tick.
  let promoted = if voters.contains(&dead_member) {
    let Some(promoted) = pick_promotable_learner(&learners, network, &raft_ack_fresh).await else {
      tracing::warn!(
        group = %group_id,
        dead_voter = %dead_member,
        downtime = ?downtime,
        "voter is down past the replace timeout but no connected learner is available to promote"
      );
      return Ok(());
    };
    Some(promoted)
  } else {
    None
  };

  let group_id = group_id.to_string();
  let raft = raft.clone();
  let network = network.clone();
  tokio::spawn(async move {
    // Owns the fence permit for the full change window; released on drop.
    let _fence = fence;
    let result = execute_membership_change(
      &group_id,
      &raft,
      &network,
      &voters,
      &member_ids,
      &dead_member,
      downtime,
      promoted,
    )
    .await;
    if let Err(err) = result {
      tracing::warn!(
        group = %group_id,
        dead_member = %dead_member,
        error = ?err,
        "asynchronous membership change failed; the guard will re-propose on a later tick"
      );
    }
  });

  Ok(())
}

/// The actual membership write, run on a background task so the guard tick
/// never blocks on `change_membership` (up to [`MEMBERSHIP_CHANGE_TIMEOUT`]).
#[allow(clippy::too_many_arguments)]
async fn execute_membership_change(
  group_id: &str,
  raft: &Raft,
  network: &Libp2pNetworkFactory,
  voters: &BTreeSet<NodeId>,
  member_ids: &BTreeSet<NodeId>,
  dead_member: &NodeId,
  downtime: Duration,
  promoted: Option<NodeId>,
) -> anyhow::Result<()> {
  match promoted {
    Some(promoted) => {
      // Dead voter: replace it with a connected learner in one membership
      // change so the quorum math stays sound.
      let mut new_voters = voters.clone();
      new_voters.remove(dead_member);
      new_voters.insert(promoted.clone());

      tracing::warn!(
        group = %group_id,
        dead_voter = %dead_member,
        downtime = ?downtime,
        promoted_learner = %promoted,
        "replacing crashed voter with a learner"
      );

      // retain=false removes the dead voter from the membership entirely, so
      // a returning node sees itself evicted and re-joins as a learner.
      timeout(
        MEMBERSHIP_CHANGE_TIMEOUT,
        raft.change_membership(new_voters, false),
      )
      .await
      .map_err(|_| anyhow!("change_membership timed out after {MEMBERSHIP_CHANGE_TIMEOUT:?}"))?
      .map_err(|err| anyhow!("change_membership failed: {err:?}"))?;
      metrics::counter!(
        "membership_guard_replacement_total",
        "group" => group_id.to_string(),
        "kind" => "replace_voter",
      )
      .increment(1);
    }
    None => {
      // Dead learner: drop it from the membership so it no longer shows up
      // as a member; a returning node re-registers itself as a learner.
      tracing::warn!(
        group = %group_id,
        dead_learner = %dead_member,
        downtime = ?downtime,
        "removing crashed learner from the membership"
      );

      timeout(
        MEMBERSHIP_CHANGE_TIMEOUT,
        raft.change_membership(
          ChangeMembers::RemoveNodes(BTreeSet::from([dead_member.clone()])),
          false,
        ),
      )
      .await
      .map_err(|_| anyhow!("remove learner timed out after {MEMBERSHIP_CHANGE_TIMEOUT:?}"))?
      .map_err(|err| anyhow!("remove learner failed: {err:?}"))?;
      metrics::counter!(
        "membership_guard_replacement_total",
        "group" => group_id.to_string(),
        "kind" => "remove_learner",
      )
      .increment(1);
    }
  }

  backfill_learner(group_id, raft, network, member_ids, dead_member).await;
  Ok(())
}

/// Backfill the learner pool from a spare connected worker, if any.
async fn backfill_learner(
  group_id: &str,
  raft: &Raft,
  network: &Libp2pNetworkFactory,
  member_ids: &BTreeSet<NodeId>,
  dead_member: &NodeId,
) {
  match pick_spare_worker(network, member_ids, dead_member).await {
    Some((node_id, addr)) => {
      match timeout(
        MEMBERSHIP_CHANGE_TIMEOUT,
        raft.add_learner(node_id.clone(), BasicNode { addr: addr.clone() }, false),
      )
      .await
      .map_err(|_| anyhow!("add_learner timed out after {MEMBERSHIP_CHANGE_TIMEOUT:?}"))
      .and_then(|result| result.map_err(|err| anyhow!("{err:?}")))
      {
        Ok(_) => tracing::info!(
          group = %group_id,
          learner = %node_id,
          %addr,
          "backfilled learner pool from a spare worker"
        ),
        Err(err) => tracing::warn!(
          group = %group_id,
          learner = %node_id,
          error = ?err,
          "backfill add_learner failed; will not retry until another replacement happens"
        ),
      }
    }
    None => tracing::warn!(
      group = %group_id,
      "no spare connected worker available to backfill the learner pool"
    ),
  }
}

/// Connection-or-announce liveness. With on-demand connections a healthy but
/// idle peer is intentionally not connected, so raw connectedness alone would
/// mark it dead; a fresh node-announce gossip counts as alive too.
async fn node_alive(network: &Libp2pNetworkFactory, node_id: &NodeId) -> bool {
  match node_id.peer_id() {
    Ok(peer_id) => network.is_peer_alive(&peer_id).await,
    Err(_) => false,
  }
}

/// Members whose last raft heartbeat/replication ack — openraft's own
/// leader-local per-member observation (`RaftMetrics::heartbeat`) — is
/// younger than `ack_timeout` (the mesh-health-adjusted freshness window).
/// Empty when the map is absent (metrics not yet populated), which degrades
/// detection to the libp2p-only view rather than blocking it.
fn fresh_raft_acks(metrics: &RaftMetrics, ack_timeout: Duration) -> BTreeSet<NodeId> {
  let Some(heartbeat) = metrics.heartbeat.as_ref() else {
    return BTreeSet::new();
  };
  heartbeat
    .iter()
    .filter(|(_, last_ack)| {
      last_ack
        .as_ref()
        .map(|t| t.elapsed() < ack_timeout)
        .unwrap_or(false)
    })
    .map(|(id, _)| id.clone())
    .collect()
}

/// Pick a learner to promote in place of a dead voter. Prefer one that is
/// both connected AND acking raft traffic — it is about to receive a vote,
/// and a learner whose transport looks up but whose raft runtime is silent
/// would weaken the new quorum. Fall back to a merely connected learner so
/// promotion is never blocked on metrics availability.
async fn pick_promotable_learner(
  learners: &[NodeId],
  network: &Libp2pNetworkFactory,
  raft_ack_fresh: &BTreeSet<NodeId>,
) -> Option<NodeId> {
  let mut connected_only = None;
  for learner in learners {
    if !node_alive(network, learner).await {
      continue;
    }
    if raft_ack_fresh.contains(learner) {
      return Some(learner.clone());
    }
    if connected_only.is_none() {
      connected_only = Some(learner.clone());
    }
  }
  connected_only
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn healthy_mesh_keeps_base_ack_timeout() {
    assert_eq!(scaled_raft_ack_timeout(1.0), RAFT_ACK_FRESH_TIMEOUT);
  }

  #[test]
  fn degraded_mesh_stretches_ack_timeout() {
    assert_eq!(
      scaled_raft_ack_timeout(0.0),
      RAFT_ACK_FRESH_TIMEOUT.mul_f64(RAFT_ACK_TIMEOUT_MAX_SCALE)
    );
    let half = scaled_raft_ack_timeout(0.5);
    assert!(half > RAFT_ACK_FRESH_TIMEOUT);
    assert!(half < RAFT_ACK_FRESH_TIMEOUT.mul_f64(RAFT_ACK_TIMEOUT_MAX_SCALE));
  }

  #[test]
  fn out_of_range_health_is_clamped() {
    assert_eq!(scaled_raft_ack_timeout(2.0), RAFT_ACK_FRESH_TIMEOUT);
    assert_eq!(
      scaled_raft_ack_timeout(-1.0),
      RAFT_ACK_FRESH_TIMEOUT.mul_f64(RAFT_ACK_TIMEOUT_MAX_SCALE)
    );
  }
}

/// A spare worker is a known, alive libp2p peer (connected or recently
/// announcing) that is not part of the group membership (and is not the
/// voter we just removed).
async fn pick_spare_worker(
  network: &Libp2pNetworkFactory,
  member_ids: &BTreeSet<NodeId>,
  dead_voter: &NodeId,
) -> Option<(NodeId, String)> {
  let mut candidates: Vec<(NodeId, String)> = Vec::new();
  for (node_id, peer_id, addr) in network.known_nodes().await {
    if member_ids.contains(&node_id) || &node_id == dead_voter {
      continue;
    }
    if !network.is_peer_alive(&peer_id).await {
      continue;
    }
    candidates.push((node_id, addr.to_string()));
  }
  candidates.sort_by(|a, b| a.0.cmp(&b.0));
  candidates.into_iter().next()
}

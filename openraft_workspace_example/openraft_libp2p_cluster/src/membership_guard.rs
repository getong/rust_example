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
//!   - the libp2p connection view (`network.is_peer_connected`), and
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
  str::FromStr,
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
  /// How long a member (voter or learner) must stay unreachable before it is
  /// replaced (voter) or removed (learner).
  pub voter_replace_timeout: Duration,
  /// How often the guard inspects the membership.
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
const RAFT_ACK_FRESH_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn run_membership_guard(
  group_id: GroupId,
  raft: Raft,
  network: Libp2pNetworkFactory,
  config: MembershipGuardConfig,
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
        if let Err(err) = guard_tick(&group_id, &raft, &network, &config, &mut down_since).await {
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
  config: &MembershipGuardConfig,
  down_since: &mut HashMap<NodeId, Instant>,
) -> anyhow::Result<()> {
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
  // no data yet) degrades to the libp2p-only view.
  let raft_ack_fresh = fresh_raft_acks(&metrics);

  // Track for how long each member (voter or learner) has been unreachable.
  // A member is alive if EITHER observer says so: a libp2p connection is
  // visible, or it acked raft traffic recently. Recovery on either signal
  // resets the clock, so transient drops within the timeout are ignored.
  let now = Instant::now();
  for member in &member_ids {
    if member == &self_id {
      continue;
    }
    if node_connected(network, member).await || raft_ack_fresh.contains(member) {
      down_since.remove(member);
    } else {
      down_since.entry(member.clone()).or_insert(now);
    }
  }
  down_since.retain(|id, _| member_ids.contains(id) && id != &self_id);

  // Act on at most ONE member per tick; pick the longest-down one.
  let expired = down_since
    .iter()
    .map(|(id, since)| (id.clone(), now.duration_since(*since)))
    .filter(|(_, downtime)| *downtime >= config.voter_replace_timeout)
    .max_by_key(|(_, downtime)| *downtime);

  let Some((dead_member, downtime)) = expired else {
    return Ok(());
  };

  if voters.contains(&dead_member) {
    // Dead voter: replace it with a connected learner in one membership
    // change so the quorum math stays sound.
    let Some(promoted) = pick_promotable_learner(&learners, network, &raft_ack_fresh).await else {
      tracing::warn!(
        group = %group_id,
        dead_voter = %dead_member,
        downtime = ?downtime,
        "voter is down past the replace timeout but no connected learner is available to promote"
      );
      return Ok(());
    };

    let mut new_voters = voters.clone();
    new_voters.remove(&dead_member);
    new_voters.insert(promoted.clone());

    tracing::warn!(
      group = %group_id,
      dead_voter = %dead_member,
      downtime = ?downtime,
      promoted_learner = %promoted,
      "replacing crashed voter with a learner"
    );

    // retain=false removes the dead voter from the membership entirely, so a
    // returning node sees itself evicted and re-joins as a learner.
    timeout(
      MEMBERSHIP_CHANGE_TIMEOUT,
      raft.change_membership(new_voters, false),
    )
    .await
    .map_err(|_| anyhow!("change_membership timed out after {MEMBERSHIP_CHANGE_TIMEOUT:?}"))?
    .map_err(|err| anyhow!("change_membership failed: {err:?}"))?;
  } else {
    // Dead learner: drop it from the membership so it no longer shows up as
    // a member; a returning node re-registers itself as a learner.
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
  }
  down_since.remove(&dead_member);

  backfill_learner(group_id, raft, network, &member_ids, &dead_member).await;

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

async fn node_connected(network: &Libp2pNetworkFactory, node_id: &NodeId) -> bool {
  match libp2p::PeerId::from_str(node_id.as_str()) {
    Ok(peer_id) => network.is_peer_connected(&peer_id).await,
    Err(_) => false,
  }
}

/// Members whose last raft heartbeat/replication ack — openraft's own
/// leader-local per-member observation (`RaftMetrics::heartbeat`) — is
/// younger than [`RAFT_ACK_FRESH_TIMEOUT`]. Empty when the map is absent
/// (metrics not yet populated), which degrades detection to the libp2p-only
/// view rather than blocking it.
fn fresh_raft_acks(metrics: &RaftMetrics) -> BTreeSet<NodeId> {
  let Some(heartbeat) = metrics.heartbeat.as_ref() else {
    return BTreeSet::new();
  };
  heartbeat
    .iter()
    .filter(|(_, last_ack)| {
      last_ack
        .as_ref()
        .map(|t| t.elapsed() < RAFT_ACK_FRESH_TIMEOUT)
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
    if !node_connected(network, learner).await {
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

/// A spare worker is a known, connected libp2p peer that is not part of the
/// group membership (and is not the voter we just removed).
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
    if !network.is_peer_connected(&peer_id).await {
      continue;
    }
    candidates.push((node_id, addr.to_string()));
  }
  candidates.sort_by(|a, b| a.0.cmp(&b.0));
  candidates.into_iter().next()
}

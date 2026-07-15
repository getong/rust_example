//! Ongoing node maintenance: post-startup verification that every group
//! elects a leader, periodic self-announcement on gossipsub, and pruning of
//! long-dead non-member nodes from the address book.

use std::{
  collections::{HashMap, HashSet},
  time::Duration,
};

use openraft::async_runtime::WatchReceiver;

use super::*;
use crate::{
  GroupId, NodeId,
  network::{
    swarm::{KvClient, NODE_ANNOUNCE_TOPIC},
    transport::Libp2pNetworkFactory,
  },
};

/// Post-startup verification of the openraft groups. Runs until every group
/// reports an elected leader (then exits), logging progress so an
/// out-of-order full-cluster restart (learners up before the voters) is
/// visible instead of silently hanging. For nodes that booted in control
/// mode it also detects the "evicted while offline" case via an
/// authoritative remote view and shuts the node down so the next start can
/// wipe the stale data and re-join as a learner.
pub(crate) async fn run_openraft_startup_verifier(
  registry: crate::GroupRegistry,
  self_id: NodeId,
  group_ids: Vec<GroupId>,
  network: Libp2pNetworkFactory,
  boot_as_control: bool,
  shutdown_tx: crate::signal::ShutdownTx,
  mut shutdown_rx: crate::signal::ShutdownRx,
) {
  let started = tokio::time::Instant::now();
  let mut pending = group_ids;
  let mut last_warn = tokio::time::Instant::now();

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => return,
      _ = tokio::time::sleep(STARTUP_VERIFY_POLL_INTERVAL) => {}
    }

    let mut still_pending = Vec::new();
    for group_id in pending {
      let Some(group) = registry.get(&group_id) else {
        still_pending.push(group_id);
        continue;
      };

      // Local leadership is live engine state, so it is trustworthy.
      // `current_leader` is NOT: openraft restores it from the persisted
      // vote, so after a restart it can name a leader that is still down.
      let local_is_leader = {
        let metrics_rx = group.raft.metrics();
        metrics_rx.borrow_watched().state.is_leader()
      };
      if local_is_leader {
        tracing::info!(
          group = %group_id,
          elapsed = ?started.elapsed(),
          "openraft group startup verified: this node is the live leader"
        );
        continue;
      }

      // Otherwise require a LIVE leader elsewhere: a remote node whose
      // metrics claim current leadership.
      if let Some(leader_metrics) =
        fetch_authoritative_group_metrics(&group_id, &self_id, &network).await
      {
        let membership = leader_metrics.membership_config.membership();
        let in_membership = membership.get_node(&self_id).is_some();

        // A control node that was evicted while offline can never rejoin
        // its stale config; restart cleanly so the next boot wipes the
        // stale data and re-joins as a learner.
        if boot_as_control && membership.nodes().next().is_some() && !in_membership {
          tracing::error!(
            group = %group_id,
            node_id = %self_id,
            "this control node was removed from the openraft membership while offline; \
             shutting down so the next start wipes the stale data and re-joins as a learner"
          );
          let _ = shutdown_tx.send(());
          return;
        }

        tracing::info!(
          group = %group_id,
          leader = %leader_metrics.id,
          in_membership,
          elapsed = ?started.elapsed(),
          "openraft group startup verified: live leader confirmed"
        );
        continue;
      }

      still_pending.push(group_id);
    }

    pending = still_pending;
    if pending.is_empty() {
      tracing::info!(
        elapsed = ?started.elapsed(),
        "openraft startup verification completed: all groups have a leader"
      );
      return;
    }

    if last_warn.elapsed() >= STARTUP_NO_LEADER_WARN_INTERVAL {
      last_warn = tokio::time::Instant::now();
      tracing::warn!(
        pending_groups = ?pending,
        elapsed = ?started.elapsed(),
        "openraft groups still have no leader; if this node started before the control voters, \
         the groups will recover automatically once a quorum of voters is online"
      );
    }
  }
}

/// Announce interval for a cluster with `known_nodes` entries in the address
/// book. Below [`NODE_ANNOUNCE_SCALE_THRESHOLD`] nodes this is the base
/// interval; above it the interval stretches proportionally so the
/// cluster-wide announce rate stays roughly constant (threshold / base
/// messages per second) instead of growing O(N). Capped so a returning node
/// is still re-listed within a bounded time.
pub fn adaptive_announce_interval(known_nodes: usize) -> Duration {
  adaptive_announce_interval_with_base(NODE_ANNOUNCE_INTERVAL, known_nodes)
}

/// [`adaptive_announce_interval`] with an explicit base interval; the
/// announcer passes the hot-reloadable base from the runtime config.
pub fn adaptive_announce_interval_with_base(base: Duration, known_nodes: usize) -> Duration {
  let factor = known_nodes.div_ceil(NODE_ANNOUNCE_SCALE_THRESHOLD).max(1);
  base
    .saturating_mul(factor.min(u32::MAX as usize) as u32)
    .min(NODE_ANNOUNCE_MAX_INTERVAL)
}

/// Periodically announce this node's identity and advertise address on the
/// dedicated gossipsub topic. Peers use these announcements to (re)register
/// the node in their known-nodes address book — the reliable counterpart to
/// the pruner: a pruned node that comes back is re-listed within one
/// announce interval instead of waiting for a slow mdns re-discovery cycle.
///
/// The interval adapts to cluster size (see [`adaptive_announce_interval`])
/// and each announcement carries it, so receivers scale the sender's
/// liveness TTL to the actual cadence instead of assuming the base one.
pub(crate) async fn run_node_announcer(
  self_id: NodeId,
  advertise_addr: String,
  network: Libp2pNetworkFactory,
  kv_client: KvClient,
  mut shutdown_rx: crate::signal::ShutdownRx,
) {
  use prost::Message as _;

  loop {
    // Hot-reloadable base: a POST /config update takes effect on the next
    // announce round.
    let base = crate::runtime_config::current().node_announce_interval();
    let interval = adaptive_announce_interval_with_base(base, network.known_nodes_count().await);
    let announcement = crate::proto::raft_kv::NodeAnnouncement {
      node_id: self_id.to_string(),
      addr: advertise_addr.clone(),
      ts_unix_ms: std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default(),
      announce_interval_ms: interval.as_millis() as u64,
    };
    kv_client
      .publish_gossipsub(NODE_ANNOUNCE_TOPIC, announcement.encode_to_vec())
      .await;

    tokio::select! {
      _ = shutdown_rx.changed() => return,
      _ = tokio::time::sleep(interval) => {}
    }
  }
}

/// Prune nodes that stay disconnected past `prune_timeout` from the libp2p
/// known-nodes address book, so crashed non-member workers do not linger in
/// `/cluster` forever. Raft group members are exempt: their lifecycle belongs
/// to the membership guard, and only after the guard has removed them from
/// every group does the pruner start counting for them.
pub(crate) async fn run_known_nodes_pruner(
  registry: crate::GroupRegistry,
  self_id: NodeId,
  network: Libp2pNetworkFactory,
  mut shutdown_rx: crate::signal::ShutdownRx,
) {
  let mut down_since: HashMap<NodeId, tokio::time::Instant> = HashMap::new();
  let mut tick = tokio::time::interval(KNOWN_NODE_PRUNE_POLL_INTERVAL);
  tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => return,
      _ = tick.tick() => {}
    }

    let now = tokio::time::Instant::now();
    // Hot-reloadable: reading per tick makes a POST /config update take
    // effect on the next pass.
    let prune_timeout = crate::runtime_config::current().voter_replace_timeout();
    let mut present: HashSet<NodeId> = HashSet::new();

    for (node_id, peer_id, _addr) in network.known_nodes().await {
      if node_id == self_id {
        continue;
      }
      present.insert(node_id.clone());

      // Alive = connected OR announcing recently; with on-demand connections
      // a healthy idle node is intentionally not connected to us.
      if network.is_peer_alive(&peer_id).await {
        down_since.remove(&node_id);
        continue;
      }

      // Members (voters/learners) are handled by the membership guard.
      if is_openraft_member_of_any_group(&registry, &node_id) {
        down_since.remove(&node_id);
        continue;
      }

      let since = *down_since.entry(node_id.clone()).or_insert(now);
      let downtime = now.duration_since(since);
      if downtime >= prune_timeout {
        if network.remove_known_node(&node_id).await {
          tracing::warn!(
            node_id = %node_id,
            downtime = ?downtime,
            "pruned dead libp2p node from the known-nodes address book"
          );
        }
        down_since.remove(&node_id);
      }
    }

    down_since.retain(|id, _| present.contains(id));
  }
}

pub(crate) fn is_openraft_member_of_any_group(
  registry: &crate::GroupRegistry,
  node_id: &NodeId,
) -> bool {
  let Some(groups) = registry.all() else {
    return false;
  };
  for group in groups.values() {
    let metrics_rx = group.raft.metrics();
    let is_member = metrics_rx
      .borrow_watched()
      .membership_config
      .membership()
      .get_node(node_id)
      .is_some();
    if is_member {
      return true;
    }
  }
  false
}

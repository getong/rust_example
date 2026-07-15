//! Control-node service loop: HTTP + leader controller + sqlite cache
//! flusher, plus the demotion watcher that drops the kademlia control role
//! when this node is evicted from the voter set while running.

use std::{net::SocketAddr, time::Duration};

use anyhow::anyhow;

use super::*;
use crate::{
  GroupHandleMap, GroupId, NodeId,
  constants::{SERVICE_OPENRAFT_LEADER_WORKER, SERVICE_SQLITE_CACHE_FLUSHER},
  http, leader_controller,
  membership_guard::MembershipGuardConfig,
  network::{swarm::KvClient, transport::Libp2pNetworkFactory},
  sqlite_cache::{self, SqliteCache},
};

pub(crate) fn spawn_openraft_leader_controller(
  shutdown: &mut crate::signal::ShutdownHandler,
  groups: GroupHandleMap,
  network: Libp2pNetworkFactory,
  registry: crate::GroupRegistry,
  membership_guard_config: Option<MembershipGuardConfig>,
) -> tokio::task::JoinHandle<()> {
  let done = shutdown.push(SERVICE_OPENRAFT_LEADER_WORKER);
  let shutdown_rx = shutdown.shutdown_rx();
  tokio::spawn(async move {
    let res = leader_controller::run_leader_controller(
      groups,
      network,
      registry,
      membership_guard_config,
      Duration::from_secs(OPENRAFT_LEADER_CONTROLLER_INTERVAL_SECS),
      shutdown_rx,
    )
    .await;
    let _ = done.send(res);
  })
}

pub(crate) async fn run_control_services(
  runtime: ControlRuntime,
  http_addr: SocketAddr,
  shutdown_rx_for_ordering: crate::signal::ShutdownRx,
) -> anyhow::Result<()> {
  let sqlite_cache = if runtime.opt.disable_sqlite_cache {
    None
  } else {
    Some(SqliteCache::connect_in_db_dir(&runtime.opt.db, &runtime.opt.redis_url).await?)
  };
  if let Some(cache) = sqlite_cache.clone() {
    sqlite_cache::set_sqlite_cache(cache)
      .map_err(|_| anyhow!("global sqlite cache already initialized"))?;
  }
  let sqlite_flush_group_id = default_openraft_group_id(&runtime.registry);

  let leader_controller_groups = runtime
    .registry
    .all()
    .map(|groups| groups.as_ref().clone())
    .ok_or_else(|| anyhow!("openraft groups are not initialized"))?;
  let http_state = build_http_state(
    &runtime.opt,
    &runtime.identity,
    &runtime.libp2p,
    runtime.registry.clone(),
    sqlite_cache.clone(),
    http::TaskFrontend::Control,
  );

  let mut shutdown = linked_shutdown(shutdown_rx_for_ordering.clone());
  let _http_handle = spawn_http(&mut shutdown, http_addr, http_state);
  let membership_guard_config = runtime
    .opt
    .auto_heal_membership
    .then(|| MembershipGuardConfig {
      tick_interval: Duration::from_secs(MEMBERSHIP_GUARD_TICK_SECS),
    });
  let _leader_controller_handle = spawn_openraft_leader_controller(
    &mut shutdown,
    leader_controller_groups,
    runtime.libp2p.network.clone(),
    runtime.registry.clone(),
    membership_guard_config,
  );

  let _sqlite_flusher_handle = sqlite_cache.map(|_| {
    spawn_sqlite_cache_flusher(
      &mut shutdown,
      runtime.opt.id.clone(),
      sqlite_flush_group_id,
      runtime.libp2p.network.clone(),
      runtime.libp2p.kv_client.clone(),
      runtime.registry.clone(),
    )
  });

  // Counterpart of the worker-side promotion watcher: demote the kademlia
  // role if this node gets evicted from the voter set while running.
  if runtime.opt.auto_heal_membership {
    tokio::spawn(run_control_demotion_watcher(
      runtime.clone(),
      shutdown_rx_for_ordering.clone(),
    ));
  }

  let (_tx, _rx, results) = shutdown.await_any_then_shutdown().await;

  let mut errors = Vec::new();
  for (service, res) in results {
    if let Err(err) = res {
      tracing::error!(service, error = ?err, "control service failed");
      errors.push(anyhow!("control service failed in {service}: {err}"));
    }
  }

  match errors.len() {
    0 => Ok(()),
    1 => Err(errors.remove(0)),
    _ => {
      let mut msg = String::new();
      use std::fmt::Write as _;
      let _ = writeln!(&mut msg, "control service errors: {}", errors.len());
      for err in errors {
        let _ = writeln!(&mut msg, "  {err}");
      }
      Err(anyhow!(msg))
    }
  }
}

/// Watch for this node being evicted from the voter set WHILE the process
/// keeps running (e.g. it was partitioned past the voter-replace timeout and
/// the membership guard replaced it, then connectivity came back).
///
/// Why this matters for kad: the control role advertises kademlia Server
/// mode plus the `openraft_cluster` provider record, and the provider record
/// is re-published every [`OPENRAFT_KAD_PROVIDER_PUBLICATION_INTERVAL`] —
/// so without this watcher an evicted-but-alive node keeps advertising
/// itself as a control node FOREVER and workers keep discovering it as a
/// task-RPC target. Raft itself is not affected by the kad mode (mode
/// changes never close connections; raft RPC uses the address book, not
/// kad lookups), which is exactly why nothing else notices the stale role.
///
/// Demotion is only performed on strong evidence: the LIVE leader of every
/// group (`state.is_leader()`, not the possibly-stale persisted
/// `current_leader`) must report that this node is not a voter, for
/// [`CONTROL_DEMOTION_CONFIRMATIONS`] consecutive rounds. On demotion the
/// node stops providing and drops to kad Client; a restart then completes
/// the self-heal (startup wipes the removed groups and rejoins as learner).
pub(crate) async fn run_control_demotion_watcher(
  runtime: ControlRuntime,
  mut shutdown_rx: crate::signal::ShutdownRx,
) {
  let mut tick = tokio::time::interval(Duration::from_secs(CONTROL_DEMOTION_POLL_INTERVAL_SECS));
  // If this very process was suspended (the main scenario that leads to
  // eviction), the default Burst behavior would fire all missed ticks
  // back-to-back on resume and collapse the confirmation rounds into one
  // instant. Delay keeps confirmations a full interval apart.
  tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
  tick.tick().await;
  let mut confirmations = 0u32;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => return,
      _ = tick.tick() => {}
    }

    if !confirmed_evicted_from_all_groups(&runtime).await {
      confirmations = 0;
      continue;
    }

    confirmations += 1;
    tracing::warn!(
      node_id = %runtime.opt.id,
      confirmations,
      required = CONTROL_DEMOTION_CONFIRMATIONS,
      "live leaders report this control node is no longer a voter"
    );
    if confirmations < CONTROL_DEMOTION_CONFIRMATIONS {
      continue;
    }

    match runtime.libp2p.client.leave_openraft_kad().await {
      Ok(()) => tracing::warn!(
        node_id = %runtime.opt.id,
        "node was evicted from the control membership while running; \
         dropped kademlia to Client mode and stopped providing the \
         control-node key. Restart this node to wipe stale group data \
         and rejoin as a learner."
      ),
      Err(err) => tracing::warn!(
        node_id = %runtime.opt.id,
        error = ?err,
        "failed to demote kademlia role after eviction"
      ),
    }
    return;
  }
}

/// True only when EVERY group has a live leader whose membership excludes
/// this node from the voter set. Unreachable groups or groups without a
/// live leader yield false — never demote on missing evidence.
pub(crate) async fn confirmed_evicted_from_all_groups(runtime: &ControlRuntime) -> bool {
  for group_id in &runtime.group_ids {
    let Some(metrics) =
      fetch_authoritative_group_metrics(group_id, &runtime.opt.id, &runtime.libp2p.network).await
    else {
      return false;
    };
    if metrics
      .membership_config
      .membership()
      .voter_ids()
      .any(|id| id == runtime.opt.id)
    {
      return false;
    }
  }
  true
}

pub(crate) fn spawn_sqlite_cache_flusher(
  shutdown: &mut crate::signal::ShutdownHandler,
  local_node_id: NodeId,
  group_id: GroupId,
  network: Libp2pNetworkFactory,
  kv_client: KvClient,
  registry: crate::GroupRegistry,
) -> tokio::task::JoinHandle<()> {
  let done = shutdown.push(SERVICE_SQLITE_CACHE_FLUSHER);
  let shutdown_rx = shutdown.shutdown_rx();
  tokio::spawn(async move {
    sqlite_cache::run_sqlite_flush_worker(
      local_node_id,
      group_id,
      network,
      kv_client,
      registry,
      Duration::from_secs(SQLITE_CACHE_FLUSH_INTERVAL_SECS),
      shutdown_rx,
    )
    .await;
    let _ = done.send(Ok(()));
  })
}

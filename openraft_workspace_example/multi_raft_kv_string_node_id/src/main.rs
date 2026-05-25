use std::{
  collections::{BTreeMap, BTreeSet},
  time::Duration,
};

use multi_raft_kv_string_node_id::{
  GroupId, NodeId, TypeConfig, create_node, groups, random_node_id, router::Router, typ,
};
use openraft::{BasicNode, async_runtime::WatchReceiver, type_config::TypeConfigExt};
use tracing_subscriber::EnvFilter;

const EXIT_AFTER_START_ARG: &str = "--exit-after-start";

fn main() -> Result<(), String> {
  init_tracing();

  let run_forever = !std::env::args().any(|arg| arg == EXIT_AFTER_START_ARG);
  TypeConfig::run(start_cluster(run_forever))
}

fn init_tracing() {
  let filter = EnvFilter::try_from_default_env()
    .unwrap_or_else(|_| EnvFilter::new("error,multi_raft_kv_string_node_id=info"));

  let _ = tracing_subscriber::fmt()
    .with_target(true)
    .with_thread_ids(true)
    .with_level(true)
    .with_env_filter(filter)
    .try_init();
}

/// Demonstrates the full membership lifecycle across three phases:
///
/// Phase 1 — form a 3-node cluster
///   boot nodes 1-3 → init with node 1 only → add nodes 2-3 as learners
///   → promote nodes 2-3 to voters (retain=true)
///   membership: {} → {n1} → {n1,n2,n3}
///
/// Phase 2 — scale up to a 5-node cluster
///   boot nodes 4-5 → add as learners → promote to voters (retain=true)
///   membership: {n1,n2,n3} → {n1,n2,n3,n4,n5}
///
/// Phase 3 — scale down back to 3 nodes
///   change_membership({n1,n2,n3}, retain=false)   ← replaces the whole set
///   membership: {n1,n2,n3,n4,n5} → {n1,n2,n3}
async fn start_cluster(run_forever: bool) -> Result<(), String> {
  let router = Router::new();
  let group_ids = groups::all();

  // ── Phase 1: form a 3-node cluster ──────────────────────────────
  let initial_ids = vec![random_node_id(), random_node_id(), random_node_id()];

  println!("╔══════════════════════════════════════════════════════════════╗");
  println!("║  Phase 1 — Start a 3-node cluster                           ║");
  println!("╚══════════════════════════════════════════════════════════════╝");
  for (i, id) in initial_ids.iter().enumerate() {
    println!("  node {} id = {}", i + 1, id);
  }
  println!("  groups = {}\n", group_ids.join(", "));

  // Start nodes 1-3 and collect their Raft handles.
  // all_rafts[i][j] = Raft instance for node i, group j.
  let mut all_rafts = start_nodes(&router, &group_ids, &initial_ids).await;

  TypeConfig::sleep(Duration::from_millis(200)).await;

  // Step 1a: single-node init — node 1 is the sole voter to start with
  initialize_single_node_groups(&all_rafts[0], &group_ids, &initial_ids[0]).await?;

  TypeConfig::sleep(Duration::from_millis(500)).await;

  // Step 1b: add nodes 2 & 3 as learners so they begin replicating logs
  add_learners(&all_rafts[0], &group_ids, &initial_ids[1 ..]).await?;

  TypeConfig::sleep(Duration::from_millis(500)).await;

  // Step 1c: promote nodes 1-3 to voters (full desired voter set).
  //   change_membership replaces the voter set with the given set.
  //   We must pass ALL nodes we want as voters, not just the new ones.
  //   result: {n1(init)} → voters={n1,n2,n3}
  promote_to_voters(&all_rafts[0], &group_ids, &initial_ids).await?;

  TypeConfig::sleep(Duration::from_millis(1_000)).await;
  write_records(&all_rafts[0], &group_ids, "phase1").await?;

  TypeConfig::sleep(Duration::from_millis(500)).await;
  println!("\n── Phase 1 complete ──");
  print_cluster_status(&all_rafts, &group_ids, &initial_ids);

  // ── Phase 2: scale up to 5-node cluster ──────────────────────────────────
  let joining_ids = vec![random_node_id(), random_node_id()];

  println!("\n╔══════════════════════════════════════════════════════════════╗");
  println!("║  Phase 2 — Scale up: add 2 nodes → 5-node cluster           ║");
  println!("╚══════════════════════════════════════════════════════════════╝");
  for (i, id) in joining_ids.iter().enumerate() {
    println!("  new node {} id = {}", i + 4, id);
  }
  println!();

  // Boot nodes 4 & 5 and append their Raft handles to all_rafts.
  let new_rafts = start_nodes(&router, &group_ids, &joining_ids).await;
  all_rafts.extend(new_rafts);

  TypeConfig::sleep(Duration::from_millis(200)).await;

  // Build the complete 5-node ID list before the membership change so we can
  // pass the FULL desired voter set to change_membership.
  let all_ids: Vec<NodeId> = initial_ids
    .iter()
    .chain(joining_ids.iter())
    .cloned()
    .collect();

  // Step 2a: add new nodes as learners so they catch up on existing log entries
  add_learners(&all_rafts[0], &group_ids, &joining_ids).await?;

  TypeConfig::sleep(Duration::from_millis(500)).await;

  // Step 2b: promote all 5 nodes to voters.
  //   change_membership REPLACES the voter set with exactly the given set.
  //   Passing only {n4,n5} would set voters={n1(leader kept),n4,n5} and
  //   demote n2,n3 to learners — NOT what we want.
  //   Passing the full {n1,n2,n3,n4,n5} gives us 5 voters.
  //   result: {n1,n2,n3} → voters={n1,n2,n3,n4,n5}
  promote_to_voters(&all_rafts[0], &group_ids, &all_ids).await?;

  TypeConfig::sleep(Duration::from_millis(1_000)).await;
  write_records(&all_rafts[0], &group_ids, "phase2").await?;

  TypeConfig::sleep(Duration::from_millis(500)).await;
  println!("\n── Phase 2 complete ──");
  print_cluster_status(&all_rafts, &group_ids, &all_ids);

  // ── Phase 3: scale down — remove nodes 4 & 5 ─────────────────────────────
  println!("\n╔══════════════════════════════════════════════════════════════╗");
  println!("║  Phase 3 — Scale down: remove 2 nodes → 3-node cluster      ║");
  println!("╚══════════════════════════════════════════════════════════════╝");
  println!("  removing: {}", joining_ids.join(", "));
  println!();

  // change_membership({n1,n2,n3}, retain=false)
  // retain=false means the voter set is REPLACED with exactly the given nodes,
  // so nodes 4 & 5 are excluded and therefore removed from the cluster.
  // result: {n1,n2,n3,n4,n5} → {n1,n2,n3}
  remove_voters(&all_rafts[0], &group_ids, &initial_ids).await?;

  TypeConfig::sleep(Duration::from_millis(1_000)).await;
  write_records(&all_rafts[0], &group_ids, "phase3").await?;

  TypeConfig::sleep(Duration::from_millis(500)).await;
  println!("\n── Phase 3 complete ──");
  // Only show the 3 remaining nodes; nodes 4-5 have been evicted.
  let n = initial_ids.len();
  print_cluster_status(&all_rafts[.. n], &group_ids, &initial_ids);

  if run_forever {
    println!("\ncluster is running; press Ctrl-C to stop");
    loop {
      TypeConfig::sleep(Duration::from_secs(30)).await;
      print_cluster_status(&all_rafts[.. n], &group_ids, &initial_ids);
    }
  }

  Ok(())
}

/// Boot `node_ids` nodes, register them on the router, and return a
/// `Vec<Vec<typ::Raft>>` indexed as `[node_index][group_index]`.
async fn start_nodes(
  router: &Router,
  group_ids: &[GroupId],
  node_ids: &[NodeId],
) -> Vec<Vec<typ::Raft>> {
  let mut node_rafts = Vec::with_capacity(node_ids.len());

  for node_id in node_ids {
    let node = create_node(node_id.clone(), group_ids, router.clone()).await;
    let rafts = group_ids
      .iter()
      .map(|group_id| {
        node
          .get_raft(group_id)
          .unwrap_or_else(|| panic!("raft group {group_id} is missing"))
          .clone()
      })
      .collect();

    TypeConfig::spawn(node.run());
    node_rafts.push(rafts);
  }

  node_rafts
}

/// Bootstrap every group with `first_node_id` as the sole initial voter.
async fn initialize_single_node_groups(
  leader_rafts: &[typ::Raft],
  group_ids: &[GroupId],
  first_node_id: &NodeId,
) -> Result<(), String> {
  let mut nodes = BTreeMap::new();
  nodes.insert(
    first_node_id.clone(),
    BasicNode {
      addr: String::new(),
    },
  );

  for (group_id, raft) in group_ids.iter().zip(leader_rafts) {
    raft
      .initialize(nodes.clone())
      .await
      .map_err(|e| format!("initialize group {group_id}: {e}"))?;
    println!("  [init]  group '{group_id}' — sole voter: {first_node_id}");
  }

  Ok(())
}

/// Add nodes as non-voting learners.  They replicate logs but cannot vote.
/// Passing `true` (blocking) waits until the learner has caught up before
/// returning, which makes the subsequent membership change safer.
async fn add_learners(
  leader_rafts: &[typ::Raft],
  group_ids: &[GroupId],
  learner_ids: &[NodeId],
) -> Result<(), String> {
  for (group_id, raft) in group_ids.iter().zip(leader_rafts) {
    for learner_id in learner_ids {
      raft
        .add_learner(
          learner_id.clone(),
          BasicNode {
            addr: String::new(),
          },
          true,
        )
        .await
        .map_err(|e| format!("add learner {learner_id} to group {group_id}: {e}"))?;

      println!("  [join]  group '{group_id}' — {learner_id} added as LEARNER");
    }
  }

  Ok(())
}

/// Set the voter membership to exactly `voter_ids`.
///
/// `change_membership(BTreeSet, retain)` **replaces** the entire voter set with
/// the given set.  Nodes excluded from the set but still present in the cluster
/// will be demoted to learners (`retain=true`) or removed entirely (`retain=false`).
///
/// Therefore the caller **must** pass the FULL set of all desired voters, not
/// just the newly joining nodes.
///
///   e.g.  current={n1},       call with {n1,n2,n3}       → result={n1,n2,n3}
///   e.g.  current={n1,n2,n3}, call with {n1,n2,n3,n4,n5} → result={n1,n2,n3,n4,n5}
async fn promote_to_voters(
  leader_rafts: &[typ::Raft],
  group_ids: &[GroupId],
  voter_ids: &[NodeId], // FULL desired voter set
) -> Result<(), String> {
  let new_voters: BTreeSet<NodeId> = voter_ids.iter().cloned().collect();

  for (group_id, raft) in group_ids.iter().zip(leader_rafts) {
    raft
      .change_membership(new_voters.clone(), true)
      .await
      .map_err(|e| format!("promote voters in group {group_id}: {e}"))?;

    println!(
      "  [join]  group '{group_id}' — voter set replaced with {} node(s): {:?}",
      new_voters.len(),
      new_voters
    );
  }

  Ok(())
}

/// Shrink the cluster to exactly `remaining_voter_ids`, removing all other nodes.
///
/// Uses `change_membership(ids, retain=false)`:
///   the voter set is **replaced** entirely with the given nodes — any node
///   not present in the set is evicted from the cluster.
///
///   e.g.  current={n1,n2,n3,n4,n5}, call with {n1,n2,n3}
///         → result={n1,n2,n3}   (n4 and n5 are removed)
async fn remove_voters(
  leader_rafts: &[typ::Raft],
  group_ids: &[GroupId],
  remaining_voter_ids: &[NodeId],
) -> Result<(), String> {
  let remaining: BTreeSet<NodeId> = remaining_voter_ids.iter().cloned().collect();

  for (group_id, raft) in group_ids.iter().zip(leader_rafts) {
    raft
      .change_membership(remaining.clone(), false)
      .await
      .map_err(|e| format!("remove voters from group {group_id}: {e}"))?;

    println!(
      "  [leave] group '{group_id}' — voter set replaced with {} node(s): {:?}",
      remaining.len(),
      remaining
    );
  }

  Ok(())
}

/// Write a marker key to every group to confirm the cluster accepts writes.
async fn write_records(
  leader_rafts: &[typ::Raft],
  group_ids: &[GroupId],
  phase: &str,
) -> Result<(), String> {
  for (group_id, raft) in group_ids.iter().zip(leader_rafts) {
    raft
      .client_write(types_kv::Request::set(
        format!("cluster:{group_id}:{phase}"),
        "ok",
      ))
      .await
      .map_err(|e| format!("write record for group {group_id}: {e}"))?;

    println!("  wrote  cluster:{group_id}:{phase}=ok");
  }

  Ok(())
}

fn print_cluster_status(node_rafts: &[Vec<typ::Raft>], group_ids: &[GroupId], node_ids: &[NodeId]) {
  println!("\ncluster status");
  for (group_index, group_id) in group_ids.iter().enumerate() {
    println!("  group '{group_id}'");

    for (node_index, node_id) in node_ids.iter().enumerate() {
      let metrics = node_rafts[node_index][group_index]
        .metrics()
        .borrow_watched()
        .clone();

      println!(
        "    node {} ({:.8}…) state={:?} leader={:?} last_applied={:?}",
        node_index + 1,
        node_id,
        metrics.state,
        metrics.current_leader,
        metrics.last_applied
      );
    }
  }
}

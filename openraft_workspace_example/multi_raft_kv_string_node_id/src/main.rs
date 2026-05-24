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

async fn start_cluster(run_forever: bool) -> Result<(), String> {
  let router = Router::new();
  let group_ids = groups::all();
  let node_ids = vec![random_node_id(), random_node_id(), random_node_id()];

  println!("starting multi-raft cluster");
  for (index, node_id) in node_ids.iter().enumerate() {
    println!("  node {} id = {}", index + 1, node_id);
  }
  println!("  groups = {}\n", group_ids.join(", "));

  let node_rafts = start_nodes(&router, &group_ids, &node_ids).await;

  TypeConfig::sleep(Duration::from_millis(200)).await;
  initialize_single_node_groups(&node_rafts[0], &group_ids, &node_ids[0]).await?;

  TypeConfig::sleep(Duration::from_millis(500)).await;
  add_learners(&node_rafts[0], &group_ids, &node_ids[1 ..]).await?;

  TypeConfig::sleep(Duration::from_millis(500)).await;
  change_membership_to_join_cluster(&node_rafts[0], &group_ids, &node_ids[.. 1], &node_ids[1 ..])
    .await?;

  TypeConfig::sleep(Duration::from_millis(1_000)).await;
  write_startup_records(&node_rafts[0], &group_ids).await?;

  TypeConfig::sleep(Duration::from_millis(500)).await;
  print_cluster_status(&node_rafts, &group_ids, &node_ids);

  if run_forever {
    println!("\ncluster is running; press Ctrl-C to stop");
    loop {
      TypeConfig::sleep(Duration::from_secs(30)).await;
      print_cluster_status(&node_rafts, &group_ids, &node_ids);
    }
  }

  Ok(())
}

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
    println!("initialized group '{group_id}' with node {first_node_id} as the only voter");
  }

  Ok(())
}

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

      println!("added node {learner_id} as learner to group '{group_id}'");
    }
  }

  Ok(())
}

/// Promote learner nodes to voting members using `retain=true`.
/// This **adds** the given nodes to the existing membership without replacing it.
async fn change_membership_to_join_cluster(
  leader_rafts: &[typ::Raft],
  group_ids: &[GroupId],
  _current_voter_ids: &[NodeId],
  joining_node_ids: &[NodeId],
) -> Result<(), String> {
  let joining = joining_node_ids.iter().cloned().collect::<BTreeSet<_>>();

  for (group_id, raft) in group_ids.iter().zip(leader_rafts) {
    raft
      .change_membership(joining.clone(), true)
      .await
      .map_err(|e| format!("join voters for group {group_id}: {e}"))?;

    println!(
      "joined {} learner nodes into group '{group_id}' (retaining existing members)",
      joining_node_ids.len(),
    );
  }

  Ok(())
}

/// Remove nodes from the cluster using `retain=false`.
/// This **replaces** the entire membership with the given set, effectively removing any node not in
/// the set.
#[allow(dead_code)]
async fn change_membership_to_leave_cluster(
  leader_rafts: &[typ::Raft],
  group_ids: &[GroupId],
  remaining_voter_ids: &[NodeId],
) -> Result<(), String> {
  let remaining = remaining_voter_ids.iter().cloned().collect::<BTreeSet<_>>();

  for (group_id, raft) in group_ids.iter().zip(leader_rafts) {
    raft
      .change_membership(remaining.clone(), false)
      .await
      .map_err(|e| format!("remove voters from group {group_id}: {e}"))?;

    println!(
      "removed nodes from group '{group_id}'; remaining voters are {:?}",
      remaining
    );
  }

  Ok(())
}

async fn write_startup_records(
  leader_rafts: &[typ::Raft],
  group_ids: &[GroupId],
) -> Result<(), String> {
  for (group_id, raft) in group_ids.iter().zip(leader_rafts) {
    raft
      .client_write(types_kv::Request::set(
        format!("cluster:{group_id}"),
        "started",
      ))
      .await
      .map_err(|e| format!("write startup record for group {group_id}: {e}"))?;
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
        "    node {} ({}) state={:?} leader={:?} last_applied={:?}",
        node_index + 1,
        node_id,
        metrics.state,
        metrics.current_leader,
        metrics.last_applied
      );
    }
  }
}

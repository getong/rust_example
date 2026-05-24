use std::{collections::BTreeMap, time::Duration};

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
    .unwrap_or_else(|_| EnvFilter::new("warn,multi_raft_kv_string_node_id=info"));

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
  initialize_groups(&node_rafts[0], &group_ids, &node_ids).await?;

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

async fn initialize_groups(
  leader_rafts: &[typ::Raft],
  group_ids: &[GroupId],
  node_ids: &[NodeId],
) -> Result<(), String> {
  let all_nodes = node_ids
    .iter()
    .map(|node_id| {
      (
        node_id.clone(),
        BasicNode {
          addr: String::new(),
        },
      )
    })
    .collect::<BTreeMap<_, _>>();

  for (group_id, raft) in group_ids.iter().zip(leader_rafts) {
    raft
      .initialize(all_nodes.clone())
      .await
      .map_err(|e| format!("initialize group {group_id}: {e}"))?;
    println!(
      "initialized group '{group_id}' with {} voters",
      node_ids.len()
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

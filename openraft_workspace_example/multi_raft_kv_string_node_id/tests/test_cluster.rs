//! Integration test for Multi-Raft KV store with 3 groups.
//!
//! This test demonstrates the TRUE Multi-Raft pattern:
//! - Each Node has ONE shared connection (not per-group connections)
//! - Multiple Raft groups share this connection
//! - Messages are routed to the correct group based on group_id

use std::{backtrace::Backtrace, collections::BTreeMap, panic::PanicHookInfo, time::Duration};

use multi_raft_kv_string_node_id::{
  GroupId, TypeConfig, create_node, groups, kv::Request, random_node_id, router::Router, typ,
};
use openraft::{BasicNode, async_runtime::WatchReceiver, type_config::TypeConfigExt};
use tracing_subscriber::EnvFilter;

pub fn log_panic(panic: &PanicHookInfo) {
  let backtrace = format!("{:?}", Backtrace::force_capture());

  eprintln!("{}", panic);

  if let Some(location) = panic.location() {
    tracing::error!(
        message = %panic,
        backtrace = %backtrace,
        panic.file = location.file(),
        panic.line = location.line(),
        panic.column = location.column(),
    );
    eprintln!(
      "{}:{}:{}",
      location.file(),
      location.line(),
      location.column()
    );
  } else {
    tracing::error!(message = %panic, backtrace = %backtrace);
  }

  eprintln!("{}", backtrace);
}

/// Test Multi-Raft cluster with 3 groups and 2 nodes.
#[test]
fn test_multi_raft_cluster() {
  TypeConfig::run(async {
    std::panic::set_hook(Box::new(|panic| {
      log_panic(panic);
    }));

    tracing_subscriber::fmt()
      .with_target(true)
      .with_thread_ids(true)
      .with_level(true)
      .with_ansi(false)
      .with_env_filter(EnvFilter::from_default_env())
      .init();

    // Shared router - this is where connection sharing happens
    let router = Router::new();
    let group_ids = groups::all();
    let node1_id = random_node_id();
    let node2_id = random_node_id();

    // Create nodes - each node has ONE connection, multiple groups
    let node1 = create_node(node1_id.clone(), &group_ids, router.clone()).await;
    let node2 = create_node(node2_id.clone(), &group_ids, router.clone()).await;

    // Get Raft handles before moving nodes into tasks
    let node1_rafts: Vec<_> = group_ids
      .iter()
      .map(|g| node1.get_raft(g).unwrap().clone())
      .collect();
    let node2_rafts: Vec<_> = group_ids
      .iter()
      .map(|g| node2.get_raft(g).unwrap().clone())
      .collect();

    // Spawn node message handlers (one per node, not per group!)
    TypeConfig::spawn(node1.run());
    TypeConfig::spawn(node2.run());

    run_test(&node1_rafts, &node2_rafts, &group_ids, &node1_id, &node2_id).await;
  });
}

async fn run_test(
  node1_rafts: &[typ::Raft],
  node2_rafts: &[typ::Raft],
  group_ids: &[GroupId],
  node1_id: &str,
  node2_id: &str,
) {
  // Wait for servers to start up
  TypeConfig::sleep(Duration::from_millis(200)).await;

  println!("\n╔════════════════════════════════════════════════════════════════════╗");
  println!("║   Multi-Raft Test: 3 groups, 2 nodes, CONNECTION SHARING          ║");
  println!("╚════════════════════════════════════════════════════════════════════╝\n");

  // =========================================================================
  // Initialize each group with node 1 as leader
  // =========================================================================
  println!("=== Initializing 3 Raft groups (all on Node 1) ===\n");

  for (i, raft) in node1_rafts.iter().enumerate() {
    let mut nodes = BTreeMap::new();
    nodes.insert(
      node1_id.to_string(),
      BasicNode {
        addr: "".to_string(),
      },
    );
    raft.initialize(nodes).await.unwrap();
    println!("  ✓ Group '{}' initialized on Node 1", group_ids[i]);
  }

  TypeConfig::sleep(Duration::from_millis(500)).await;

  // =========================================================================
  // Add Node 2 as learner for each group
  // =========================================================================
  println!("\n=== Adding Node 2 as learner to all groups ===\n");

  for (i, raft) in node1_rafts.iter().enumerate() {
    let node = BasicNode {
      addr: "".to_string(),
    };
    raft
      .add_learner(node2_id.to_string(), node, true)
      .await
      .unwrap();
    println!("  ✓ Group '{}': Node 2 added as learner", group_ids[i]);
  }

  TypeConfig::sleep(Duration::from_millis(500)).await;

  // =========================================================================
  // Write data to each group
  // =========================================================================
  println!("\n=== Writing data to each group ===\n");

  // users group
  node1_rafts[0]
    .client_write(Request::set("user:1", "Alice"))
    .await
    .unwrap();
  node1_rafts[0]
    .client_write(Request::set("user:2", "Bob"))
    .await
    .unwrap();
  println!("  ✓ Group 'users': wrote user:1=Alice, user:2=Bob");

  // orders group
  node1_rafts[1]
    .client_write(Request::set("order:1001", "pending"))
    .await
    .unwrap();
  node1_rafts[1]
    .client_write(Request::set("order:1002", "shipped"))
    .await
    .unwrap();
  println!("  ✓ Group 'orders': wrote order:1001=pending, order:1002=shipped");

  // products group
  node1_rafts[2]
    .client_write(Request::set("product:A", "Widget"))
    .await
    .unwrap();
  node1_rafts[2]
    .client_write(Request::set("product:B", "Gadget"))
    .await
    .unwrap();
  println!("  ✓ Group 'products': wrote product:A=Widget, product:B=Gadget");

  TypeConfig::sleep(Duration::from_millis(500)).await;

  // =========================================================================
  // Verify replication
  // =========================================================================
  println!("\n=== Verifying replication to Node 2 ===\n");

  for (i, raft) in node2_rafts.iter().enumerate() {
    let metrics = raft.metrics().borrow_watched().clone();
    println!(
      "  Group '{}' on Node 2: last_applied={:?}",
      group_ids[i], metrics.last_applied
    );
    assert!(
      metrics.last_applied.is_some(),
      "Group {} should have applied logs",
      group_ids[i]
    );
  }
}

// ============================================================================
// Test: Leader Distribution using transfer_leader
// ============================================================================

/// Test that demonstrates using transfer_leader to distribute leaders.
#[test]
fn test_leader_distribution() {
  TypeConfig::run(async {
    let router = Router::new();
    let group_ids = groups::all();
    let node1_id = random_node_id();
    let node2_id = random_node_id();
    let node3_id = random_node_id();

    // Create 3 nodes
    let node1 = create_node(node1_id.clone(), &group_ids, router.clone()).await;
    let node2 = create_node(node2_id.clone(), &group_ids, router.clone()).await;
    let node3 = create_node(node3_id.clone(), &group_ids, router.clone()).await;

    let node1_rafts: Vec<_> = group_ids
      .iter()
      .map(|g| node1.get_raft(g).unwrap().clone())
      .collect();
    let node2_rafts: Vec<_> = group_ids
      .iter()
      .map(|g| node2.get_raft(g).unwrap().clone())
      .collect();
    let node3_rafts: Vec<_> = group_ids
      .iter()
      .map(|g| node3.get_raft(g).unwrap().clone())
      .collect();

    TypeConfig::spawn(node1.run());
    TypeConfig::spawn(node2.run());
    TypeConfig::spawn(node3.run());

    run_leader_distribution_test(
      &node1_rafts,
      &node2_rafts,
      &node3_rafts,
      &group_ids,
      &node1_id,
      &node2_id,
      &node3_id,
    )
    .await;
  });
}

async fn run_leader_distribution_test(
  node1_rafts: &[typ::Raft],
  node2_rafts: &[typ::Raft],
  node3_rafts: &[typ::Raft],
  group_ids: &[GroupId],
  node1_id: &str,
  node2_id: &str,
  node3_id: &str,
) {
  TypeConfig::sleep(Duration::from_millis(200)).await;

  println!("\n╔════════════════════════════════════════════════════════════════════╗");
  println!("║   Leader Distribution Test using transfer_leader                  ║");
  println!("╚════════════════════════════════════════════════════════════════════╝\n");

  // Initialize all groups on Node 1 with all 3 nodes as voters
  println!("=== Initializing all groups with 3 voters ===\n");

  let all_nodes = {
    let mut nodes = BTreeMap::new();
    nodes.insert(
      node1_id.to_string(),
      BasicNode {
        addr: "".to_string(),
      },
    );
    nodes.insert(
      node2_id.to_string(),
      BasicNode {
        addr: "".to_string(),
      },
    );
    nodes.insert(
      node3_id.to_string(),
      BasicNode {
        addr: "".to_string(),
      },
    );
    nodes
  };

  for (i, raft) in node1_rafts.iter().enumerate() {
    raft.initialize(all_nodes.clone()).await.unwrap();
    println!("  ✓ Group '{}' initialized (voters: 1, 2, 3)", group_ids[i]);
  }

  TypeConfig::sleep(Duration::from_millis(1000)).await;

  // Transfer leaders to distribute load
  println!("\n=== Using transfer_leader to distribute leaders ===\n");

  // orders -> Node 2
  println!("  → Transferring 'orders' leader to Node 2...");
  node1_rafts[1]
    .trigger()
    .transfer_leader(node2_id.to_string())
    .await
    .unwrap();
  TypeConfig::sleep(Duration::from_millis(1000)).await;

  // products -> Node 3
  println!("  → Transferring 'products' leader to Node 3...");
  node1_rafts[2]
    .trigger()
    .transfer_leader(node3_id.to_string())
    .await
    .unwrap();
  TypeConfig::sleep(Duration::from_millis(1000)).await;

  // Verify distribution
  println!("\n=== Verifying leader distribution ===\n");

  let users_leader = node1_rafts[0]
    .metrics()
    .borrow_watched()
    .current_leader
    .clone();
  let orders_leader = node2_rafts[1]
    .metrics()
    .borrow_watched()
    .current_leader
    .clone();
  let products_leader = node3_rafts[2]
    .metrics()
    .borrow_watched()
    .current_leader
    .clone();

  println!("  Group 'users':    leader = {:?}", users_leader);
  println!("  Group 'orders':   leader = {:?}", orders_leader);
  println!("  Group 'products': leader = {:?}", products_leader);

  assert_eq!(
    users_leader,
    Some(node1_id.to_string()),
    "users leader should be Node 1"
  );
  assert_eq!(
    orders_leader,
    Some(node2_id.to_string()),
    "orders leader should be Node 2"
  );
  assert_eq!(
    products_leader,
    Some(node3_id.to_string()),
    "products leader should be Node 3"
  );
}

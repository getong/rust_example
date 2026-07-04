use std::{io::Cursor, sync::Arc, time::Duration};

use openraft::{BasicNode, Config, async_runtime::WatchReceiver, type_config::TypeConfigExt};

use self::{app::Node, router::Router};

const DIRECT_OPENRAFT_VERSION: &str = "0.10.0-alpha.27";
const OCTOPII_VENDOR_OPENRAFT_VERSION: &str = "0.10.0";

pub mod api;
pub mod app;
pub mod log_mem;
pub mod network;
pub mod router;
pub mod sm_mem;
pub mod store;
pub mod typ;
pub mod types_kv;

/// Node ID type - identifies a node in the cluster
pub type NodeId = u64;

/// Group ID type - identifies a Raft group
pub type GroupId = String;

openraft::declare_raft_types!(
    /// Declare the type configuration for Multi-Raft K/V store.
    pub TypeConfig:
        D = self::types_kv::Request,
        R = self::types_kv::Response,
        SnapshotData = Cursor<Vec<u8>>,
);

pub type LogStore = store::LogStore;
pub type StateMachineStore = sm_mem::StateMachineStore<TypeConfig>;
pub type Raft = openraft::Raft<TypeConfig, StateMachineStore>;

pub mod groups {
  pub const USERS: &str = "users";
  pub const ORDERS: &str = "orders";
  pub const PRODUCTS: &str = "products";

  pub fn all() -> Vec<String> {
    vec![USERS.to_string(), ORDERS.to_string(), PRODUCTS.to_string()]
  }
}

pub fn encode<T: serde::Serialize>(t: T) -> String {
  serde_json::to_string(&t).expect("multi-raft demo serialization should not fail")
}

pub fn decode<T: serde::de::DeserializeOwned>(s: &str) -> T {
  serde_json::from_str(s).expect("multi-raft demo deserialization should not fail")
}

/// Create a Node with multiple Raft groups.
///
/// - One Node has ONE connection (shared by all groups)
/// - Each group has its own Raft instance
pub async fn create_node(node_id: NodeId, group_ids: &[GroupId], router: Router) -> Node {
  let (mut node, _tx) = Node::new(node_id, router.clone());

  for group_id in group_ids {
    let config = Config {
      heartbeat_interval: 500,
      election_timeout_min: 1500,
      election_timeout_max: 3000,
      max_in_snapshot_log_to_keep: 0,
      ..Default::default()
    };

    let config = Arc::new(
      config
        .validate()
        .expect("multi-raft config should be valid"),
    );
    let log_store = LogStore::default();
    let state_machine_store = StateMachineStore::default();

    let network = network::NetworkFactory::new(router.clone(), group_id.clone());

    let raft = openraft::Raft::new(
      node_id,
      config,
      network,
      log_store,
      state_machine_store.clone(),
    )
    .await
    .expect("multi-raft raft instance should start");

    node.add_group(group_id.clone(), raft, state_machine_store);
  }

  node
}

/// Run the copied multi-raft KV example inside this crate.
pub async fn run_demo() -> octopii::Result<()> {
  run_demo_inner()
    .await
    .map_err(|e| octopii::OctopiiError::Rpc(e.to_string()))
}

async fn run_demo_inner() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let router = Router::new();
  let group_ids = groups::all();

  let node1 = create_node(1, &group_ids, router.clone()).await;
  let node2 = create_node(2, &group_ids, router).await;

  let node1_rafts = rafts_for_groups(&node1, &group_ids);
  let node2_rafts = rafts_for_groups(&node2, &group_ids);

  TypeConfig::spawn(node1.run());
  TypeConfig::spawn(node2.run());

  TypeConfig::sleep(Duration::from_millis(200)).await;

  println!(
    "octopii vendors openraft@{OCTOPII_VENDOR_OPENRAFT_VERSION}; copied multi-raft code uses \
     direct openraft@{DIRECT_OPENRAFT_VERSION}"
  );
  println!("multi-raft groups share one Router connection per physical node");

  for (i, raft) in node1_rafts.iter().enumerate() {
    let mut nodes = std::collections::BTreeMap::new();
    nodes.insert(
      1_u64,
      BasicNode {
        addr: "in-memory-node-1".to_string(),
      },
    );
    raft.initialize(nodes).await?;
    println!("  group '{}' initialized on node 1", group_ids[i]);
  }

  TypeConfig::sleep(Duration::from_millis(500)).await;

  for (i, raft) in node1_rafts.iter().enumerate() {
    raft
      .add_learner(
        2,
        BasicNode {
          addr: "in-memory-node-2".to_string(),
        },
        true,
      )
      .await?;
    println!("  group '{}' added node 2 as learner", group_ids[i]);
  }

  TypeConfig::sleep(Duration::from_millis(500)).await;

  node1_rafts[0]
    .client_write(types_kv::Request::set("user:1", "Alice"))
    .await?;
  node1_rafts[1]
    .client_write(types_kv::Request::set("order:1001", "pending"))
    .await?;
  node1_rafts[2]
    .client_write(types_kv::Request::set("product:A", "Widget"))
    .await?;

  TypeConfig::sleep(Duration::from_millis(500)).await;

  for (i, raft) in node2_rafts.iter().enumerate() {
    let metrics = raft.metrics().borrow_watched().clone();
    println!(
      "  group '{}' on node 2: leader={:?}, last_applied={:?}",
      group_ids[i], metrics.current_leader, metrics.last_applied
    );
  }

  Ok(())
}

fn rafts_for_groups(node: &Node, group_ids: &[GroupId]) -> Vec<Raft> {
  group_ids
    .iter()
    .map(|group_id| {
      node
        .get_raft(group_id)
        .unwrap_or_else(|| panic!("missing raft group {group_id}"))
        .clone()
    })
    .collect()
}

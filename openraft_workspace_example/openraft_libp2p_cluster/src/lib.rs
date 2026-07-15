#![allow(clippy::uninlined_format_args)]
#![deny(unused_qualifications)]

use std::{collections::BTreeMap, sync::Arc};

use arc_swap::ArcSwap;
use once_cell::sync::Lazy;

pub mod app;
pub mod constants;
pub mod graphviz;
pub mod http;
pub mod leader_controller;
pub mod membership_guard;
pub mod network;
pub mod proto;
pub mod rocksstore_crud;
pub mod signal;
pub mod sqlite_cache;
pub mod sqlite_sync_rpc;
pub mod store;
pub mod tasks;
pub mod telemetry;
pub mod typ;
pub mod types_kv;

pub type TypeConfig = rocksstore_crud::TypeConfig;
pub type NodeId = <TypeConfig as openraft::RaftTypeConfig>::NodeId;
pub type GroupId = String;
pub type SnapshotData = std::io::Cursor<Vec<u8>>;
pub type Raft = openraft::Raft<TypeConfig, store::StateMachineStore>;
pub type Unreachable = openraft::error::Unreachable<TypeConfig>;

#[derive(Clone)]
pub struct GroupHandle {
  pub raft: Raft,
  pub kv_data: store::KvData,
}

pub type GroupHandleMap = BTreeMap<GroupId, GroupHandle>;

/// Global registry of raft groups. `ArcSwap` (instead of `OnceCell`) lets the
/// whole map be atomically replaced at runtime, so groups can be added or
/// removed without a restart (hot reconfiguration). Readers pay a lock-free
/// load; an empty map means "not initialized yet".
pub static OPENRAFT_GROUPS: Lazy<ArcSwap<GroupHandleMap>> =
  Lazy::new(|| ArcSwap::from_pointee(GroupHandleMap::new()));

/// Install or atomically replace the global group map.
pub fn set_openraft_groups(groups: GroupHandleMap) {
  OPENRAFT_GROUPS.store(Arc::new(groups));
}

pub fn openraft_groups() -> Option<Arc<GroupHandleMap>> {
  let groups = OPENRAFT_GROUPS.load_full();
  if groups.is_empty() {
    None
  } else {
    Some(groups)
  }
}

pub fn openraft_group(group_id: &str) -> Option<GroupHandle> {
  OPENRAFT_GROUPS.load().get(group_id).cloned()
}

pub mod groups {
  pub const TASKS: &str = "tasks";
  pub const USERS: &str = "users";
  pub const ORDERS: &str = "orders";
  pub const PRODUCTS: &str = "products";

  pub fn all() -> Vec<String> {
    vec![
      USERS.to_string(),
      ORDERS.to_string(),
      PRODUCTS.to_string(),
      TASKS.to_string(),
    ]
  }
}

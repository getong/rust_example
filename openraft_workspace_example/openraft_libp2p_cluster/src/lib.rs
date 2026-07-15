#![allow(clippy::uninlined_format_args)]
#![deny(unused_qualifications)]

use std::{collections::BTreeMap, sync::Arc};

use arc_swap::ArcSwap;

pub mod app;
pub mod constants;
pub mod error;
pub mod graphviz;
pub mod http;
pub mod leader_controller;
pub mod membership_guard;
pub mod network;
pub mod proto;
pub mod rocksstore_crud;
pub mod runtime_config;
pub mod signal;
pub mod sqlite_cache;
pub mod sqlite_sync_rpc;
pub mod store;
pub mod tasks;
pub mod telemetry;
pub mod typ;
pub mod types_kv;

pub use error::ClusterError;

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
  /// Per-group membership-change fence. Multi-step flows (add_learner →
  /// catch-up → change_membership) are NOT atomic in openraft; holding this
  /// lock across the whole window keeps a concurrent membership request from
  /// interleaving inside it (TOCTOU). Writers use `try_lock` and reject with
  /// "membership change in progress" instead of queueing, so a slow catch-up
  /// window never piles up blocked changes behind it.
  pub membership_fence: Arc<tokio::sync::Mutex<()>>,
}

impl GroupHandle {
  pub fn new(raft: Raft, kv_data: store::KvData) -> Self {
    Self {
      raft,
      kv_data,
      membership_fence: Arc::new(tokio::sync::Mutex::new(())),
    }
  }
}

pub type GroupHandleMap = BTreeMap<GroupId, GroupHandle>;

/// Cloneable handle to a set of raft groups: lock-free reads via `ArcSwap`,
/// atomic whole-map replacement (hot reconfiguration). Components that need
/// group access take a registry — the composition root (`app::run`) creates
/// one instance and injects it everywhere (HTTP `AppState`, swarm dispatcher,
/// schedulers), so tests construct their own isolated instance and there is
/// no process-wide global. An empty map means "not initialized yet".
#[derive(Clone, Default)]
pub struct GroupRegistry {
  groups: Arc<ArcSwap<GroupHandleMap>>,
}

impl GroupRegistry {
  pub fn new() -> Self {
    Self::default()
  }

  /// Install or atomically replace the group map.
  pub fn set(&self, groups: GroupHandleMap) {
    self.groups.store(Arc::new(groups));
  }

  pub fn all(&self) -> Option<Arc<GroupHandleMap>> {
    let groups = self.groups.load_full();
    if groups.is_empty() {
      None
    } else {
      Some(groups)
    }
  }

  pub fn get(&self, group_id: &str) -> Option<GroupHandle> {
    self.groups.load().get(group_id).cloned()
  }
}

pub mod groups {
  pub const TASKS: &str = "tasks";
  pub const USERS: &str = "users";
  pub const ORDERS: &str = "orders";
  pub const PRODUCTS: &str = "products";

  /// Env var overriding the raft group layout: a comma-separated list of
  /// group ids, e.g. `OPENRAFT_GROUP_IDS=users,inventory,tasks`.
  pub const GROUP_IDS_ENV: &str = "OPENRAFT_GROUP_IDS";

  /// Raft group ids served by this node. The four demo groups by default;
  /// configurable at startup via [`GROUP_IDS_ENV`] (also loadable from
  /// `.env`) instead of being hardcoded. The `tasks` group is always
  /// included — the task subsystem (scheduler, workers, HTTP task API)
  /// depends on it. Live groups are discoverable at runtime through
  /// [`crate::GroupRegistry`] / `GET /groups`, not this static list.
  pub fn all() -> Vec<String> {
    let mut ids: Vec<String> = match std::env::var(GROUP_IDS_ENV) {
      Ok(raw) => raw
        .split(',')
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect(),
      Err(_) => vec![USERS.to_string(), ORDERS.to_string(), PRODUCTS.to_string()],
    };
    if !ids.iter().any(|id| id == TASKS) {
      ids.push(TASKS.to_string());
    }
    ids.dedup();
    ids
  }
}

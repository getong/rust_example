#![allow(clippy::uninlined_format_args)]
#![deny(unused_qualifications)]

use std::collections::BTreeMap;

use once_cell::sync::OnceCell;

pub mod apalis_raft;
pub mod app;
pub mod constants;
pub mod http;
pub mod network;
pub mod proto;
pub mod rocksstore_crud;
pub mod signal;
pub mod store;
pub mod typ;

pub type TypeConfig = rocksstore_crud::TypeConfig;
pub type NodeId = <TypeConfig as openraft::RaftTypeConfig>::NodeId;
pub type GroupId = String;
pub type Raft = openraft::Raft<TypeConfig, store::StateMachineStore>;
pub type Unreachable = openraft::error::Unreachable<TypeConfig>;

#[derive(Clone)]
pub struct GroupHandle {
  pub raft: Raft,
  pub kv_data: store::KvData,
}

pub type GroupHandleMap = BTreeMap<GroupId, GroupHandle>;

pub static OPENRAFT_GROUPS: OnceCell<GroupHandleMap> = OnceCell::new();

pub fn set_openraft_groups(groups: GroupHandleMap) -> Result<(), GroupHandleMap> {
  OPENRAFT_GROUPS.set(groups)
}

pub fn openraft_groups() -> Option<&'static GroupHandleMap> {
  OPENRAFT_GROUPS.get()
}

pub fn openraft_group(group_id: &str) -> Option<GroupHandle> {
  openraft_groups()
    .and_then(|groups| groups.get(group_id))
    .cloned()
}

pub mod groups {
  pub const APALIS: &str = "apalis";
  pub const USERS: &str = "users";
  pub const ORDERS: &str = "orders";
  pub const PRODUCTS: &str = "products";

  pub fn all() -> Vec<String> {
    vec![
      USERS.to_string(),
      ORDERS.to_string(),
      PRODUCTS.to_string(),
      APALIS.to_string(),
    ]
  }
}

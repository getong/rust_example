#![allow(clippy::uninlined_format_args)]
#![deny(unused_qualifications)]

use std::collections::BTreeMap;

pub mod app;
pub mod constants;
pub mod http;
pub mod kameo_remote;
pub mod network;
pub mod proto;
pub mod signal;
pub mod store;
pub mod typ;

pub type TypeConfig = openraft_rocksstore_crud::TypeConfig;
pub type NodeId = u64;
pub type GroupId = String;
pub type Unreachable = openraft::error::Unreachable<TypeConfig>;

#[derive(Clone)]
pub struct GroupHandle {
  pub raft: typ::Raft,
  pub kv_data: store::KvData,
}

pub type GroupHandleMap = BTreeMap<GroupId, GroupHandle>;

pub mod groups {
  pub const USERS: &str = "users";
  pub const ORDERS: &str = "orders";
  pub const PRODUCTS: &str = "products";

  pub fn all() -> Vec<String> {
    vec![USERS.to_string(), ORDERS.to_string(), PRODUCTS.to_string()]
  }
}

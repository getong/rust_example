#![allow(clippy::uninlined_format_args)]
#![deny(unused_qualifications)]

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
pub type Unreachable = openraft::error::Unreachable<TypeConfig>;

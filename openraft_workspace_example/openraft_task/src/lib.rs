#![allow(clippy::uninlined_format_args)]
#![deny(unused_qualifications)]

pub mod apalis_raft;
pub mod app;
pub mod network;
pub mod rocksstore_crud;
pub mod store;
pub mod typ;
pub mod types_kv;

pub type TypeConfig = rocksstore_crud::TypeConfig;
pub type NodeId = u64;
pub type Raft = openraft::Raft<TypeConfig, rocksstore_crud::RocksStateMachine>;

pub use apalis_raft::DemoTask;
pub use app::{Opt, run};

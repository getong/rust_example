//! Distributed systems module combining chitchat and openraft
//!
//! This module implements the Stract pattern where:
//! - Chitchat handles cluster membership and service discovery
//! - OpenRaft provides consistent distributed hash table (DHT) operations
//! - Services can discover each other and form DHT clusters

pub mod cluster;
pub mod dht;
pub mod member;

pub use cluster::Cluster;
pub use member::{Member, Service, ShardId};

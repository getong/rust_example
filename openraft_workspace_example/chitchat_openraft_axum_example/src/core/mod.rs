//! Core Technologies Module
//! 
//! This module contains the core distributed system technologies:
//! - chitchat: Gossip protocol for membership management
//! - openraft: Distributed consensus and consistency
//! 
//! Based on Stract's architecture patterns.

pub mod chitchat_integration;

pub use chitchat_integration::{ChitchatManager, ServiceType};

/// Simple core configuration
#[derive(Debug, Clone)]
pub struct CoreConfig {
    pub cluster_name: String,
    pub node_id: String,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            cluster_name: "stract-cluster".to_string(),
            node_id: "node-1".to_string(),
        }
    }
}

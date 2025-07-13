use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Enum to specify which backend to use for operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsistencyLevel {
  /// Use Chitchat for eventual consistency
  Eventual,
  /// Use OpenRaft for strong consistency
  Strong,
  /// Use both systems with reconciliation
  Hybrid,
}

impl Default for ConsistencyLevel {
  fn default() -> Self {
    ConsistencyLevel::Eventual
  }
}

impl std::str::FromStr for ConsistencyLevel {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_lowercase().as_str() {
      "eventual" => Ok(ConsistencyLevel::Eventual),
      "strong" => Ok(ConsistencyLevel::Strong),
      "hybrid" => Ok(ConsistencyLevel::Hybrid),
      _ => Err(format!("Invalid consistency level: {}", s)),
    }
  }
}

/// Router to determine which backend to use based on the operation
#[derive(Debug, Clone)]
pub struct ConsistencyRouter {
  /// Rules for routing operations
  rules: HashMap<String, ConsistencyLevel>,
  /// Default consistency level
  default: ConsistencyLevel,
}

impl Default for ConsistencyRouter {
  fn default() -> Self {
    Self::new()
  }
}

impl ConsistencyRouter {
  pub fn new() -> Self {
    let mut rules = HashMap::new();

    // Example routing rules
    rules.insert("critical_".to_string(), ConsistencyLevel::Strong);
    rules.insert("balance_".to_string(), ConsistencyLevel::Strong);
    rules.insert("transaction_".to_string(), ConsistencyLevel::Strong);
    rules.insert("config_".to_string(), ConsistencyLevel::Strong);

    rules.insert("cache_".to_string(), ConsistencyLevel::Eventual);
    rules.insert("session_".to_string(), ConsistencyLevel::Eventual);
    rules.insert("temp_".to_string(), ConsistencyLevel::Eventual);
    rules.insert("metadata_".to_string(), ConsistencyLevel::Eventual);

    Self {
      rules,
      default: ConsistencyLevel::Eventual,
    }
  }

  /// Determine which backend to use for a given key
  pub fn route_for_key(&self, key: &str) -> ConsistencyLevel {
    // Check if key matches any prefix rules
    for (prefix, consistency) in &self.rules {
      if key.starts_with(prefix) {
        return *consistency;
      }
    }

    // Return default if no rules match
    self.default
  }

  /// Override consistency level for specific operation
  pub fn route_with_override(
    &self,
    key: &str,
    override_level: Option<ConsistencyLevel>,
  ) -> ConsistencyLevel {
    override_level.unwrap_or_else(|| self.route_for_key(key))
  }

  /// Add a new routing rule
  pub fn add_rule(&mut self, prefix: String, consistency: ConsistencyLevel) {
    self.rules.insert(prefix, consistency);
  }

  /// Remove a routing rule
  pub fn remove_rule(&mut self, prefix: &str) {
    self.rules.remove(prefix);
  }

  /// Get all current rules
  pub fn get_rules(&self) -> &HashMap<String, ConsistencyLevel> {
    &self.rules
  }

  /// Set default consistency level
  pub fn set_default(&mut self, default: ConsistencyLevel) {
    self.default = default;
  }
}

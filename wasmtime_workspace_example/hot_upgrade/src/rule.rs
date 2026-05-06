use std::{fs, path::Path};

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

use crate::types::{Decision, Request, Response, State};

#[derive(Debug, Clone, Deserialize)]
struct RuleConfig {
  version: String,
  required_state_schema: u32,
  review_threshold: i64,
  #[serde(default)]
  even_user_fast_lane_max_amount: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct RuleEngine {
  config: RuleConfig,
}

impl RuleEngine {
  pub fn load(path: impl AsRef<Path>) -> Result<Self> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path)
      .with_context(|| format!("failed to read rule file {}", path.display()))?;
    let config: RuleConfig = serde_json::from_str(&raw)
      .with_context(|| format!("failed to parse rule file {}", path.display()))?;

    let engine = Self { config };
    engine.validate_config()?;
    Ok(engine)
  }

  pub fn version(&self) -> &str {
    &self.config.version
  }

  pub fn handle(&self, state: &mut State, request: Request) -> Result<Response> {
    if state.schema_version != self.config.required_state_schema {
      return Err(anyhow!(
        "rule {} expects state schema {}, got {}",
        self.version(),
        self.config.required_state_schema,
        state.schema_version
      ));
    }

    state.processed += 1;

    let decision = if request.amount > self.config.review_threshold {
      Decision::Review
    } else if let Some(max_amount) = self.config.even_user_fast_lane_max_amount {
      if request.user_id % 2 == 0 && request.amount <= max_amount {
        state.fast_lane_hits += 1;
        Decision::AllowFastLane
      } else {
        Decision::Allow
      }
    } else {
      Decision::Allow
    };

    Ok(Response {
      decision,
      rule_version: self.config.version.clone(),
    })
  }

  pub fn migrate_state(&self, state: &mut State) -> Result<()> {
    while state.schema_version < self.config.required_state_schema {
      match state.schema_version {
        0 => {
          state.schema_version = 1;
        }
        1 => {
          // Schema 2 starts tracking fast-lane hits explicitly.
          state.fast_lane_hits = 0;
          state.schema_version = 2;
        }
        current => {
          return Err(anyhow!(
            "missing migrator for state schema {current} -> {}",
            current + 1
          ));
        }
      }
    }

    if state.schema_version > self.config.required_state_schema {
      return Err(anyhow!(
        "rule {} expects schema {}, but actor is already at schema {}",
        self.version(),
        self.config.required_state_schema,
        state.schema_version
      ));
    }

    Ok(())
  }

  fn validate_config(&self) -> Result<()> {
    if self.config.version.trim().is_empty() {
      return Err(anyhow!("rule version cannot be empty"));
    }
    if self.config.required_state_schema == 0 {
      return Err(anyhow!("required_state_schema must be >= 1"));
    }
    if self.config.review_threshold <= 0 {
      return Err(anyhow!("review_threshold must be positive"));
    }
    if let Some(max_amount) = self.config.even_user_fast_lane_max_amount {
      if max_amount <= 0 {
        return Err(anyhow!("even_user_fast_lane_max_amount must be positive"));
      }
      if max_amount > self.config.review_threshold {
        return Err(anyhow!(
          "even_user_fast_lane_max_amount cannot exceed review_threshold"
        ));
      }
    }
    Ok(())
  }
}

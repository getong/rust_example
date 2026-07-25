use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::{bindings::exports::rule as component_rule, types::Decision};

pub(crate) const SERVICE_STATE_PATH: &str = "state/service.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct State {
  pub(crate) processed: u64,
  pub(crate) schema_version: u32,
  pub(crate) allow_count: u64,
  pub(crate) review_count: u64,
  pub(crate) fast_lane_hits: u64,
  pub(crate) upgrades: u64,
  pub(crate) last_score: i32,
  pub(crate) total_score: i64,
  pub(crate) v2: Option<StateV2Stats>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StateV2Stats {
  pub(crate) migration_generation: u64,
  pub(crate) legacy_processed_at_migration: u64,
  pub(crate) fast_lane_amount: i64,
  pub(crate) reviewed_amount: i64,
  pub(crate) largest_amount: i64,
  pub(crate) high_risk_requests: u64,
  pub(crate) late_night_reviews: u64,
  pub(crate) last_decision: Decision,
  pub(crate) last_policy_id: i32,
}

impl Default for StateV2Stats {
  fn default() -> Self {
    Self {
      migration_generation: 0,
      legacy_processed_at_migration: 0,
      fast_lane_amount: 0,
      reviewed_amount: 0,
      largest_amount: 0,
      high_risk_requests: 0,
      late_night_reviews: 0,
      last_decision: Decision::Allow,
      last_policy_id: 0,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceSnapshot {
  V1(ServiceSnapshotV1),
  V2(ServiceSnapshotV2),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceSnapshotV1 {
  pub processed: u64,
  pub schema_version: u32,
  pub allow_count: u64,
  pub review_count: u64,
  pub upgrades: u64,
  pub last_score: i32,
  pub average_score: i32,
  pub current_rule_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceSnapshotV2 {
  pub processed: u64,
  pub schema_version: u32,
  pub allow_count: u64,
  pub review_count: u64,
  pub fast_lane_hits: u64,
  pub upgrades: u64,
  pub last_score: i32,
  pub average_score: i32,
  pub current_rule_version: String,
  pub migration_generation: u64,
  pub legacy_processed_at_migration: u64,
  pub fast_lane_amount: i64,
  pub reviewed_amount: i64,
  pub largest_amount: i64,
  pub high_risk_requests: u64,
  pub late_night_reviews: u64,
  pub review_rate_bps: u32,
  pub fast_lane_rate_bps: u32,
  pub last_decision: Decision,
  pub last_policy_id: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceState {
  inner: State,
}

impl ServiceState {
  pub fn migrate_to_schema(&mut self, required_schema: u32) -> Result<()> {
    while self.inner.schema_version < required_schema {
      match self.inner.schema_version {
        0 => {
          self.inner.schema_version = 1;
        }
        1 => {
          self.inner.fast_lane_hits = 0;
          self.inner.v2 = Some(StateV2Stats {
            migration_generation: self.inner.upgrades + 1,
            legacy_processed_at_migration: self.inner.processed,
            ..Default::default()
          });
          self.inner.schema_version = 2;
        }
        current => {
          return Err(anyhow!(
            "missing migrator for state schema {current} -> {}",
            current + 1
          ));
        }
      }
    }

    if self.inner.schema_version > required_schema {
      return Err(anyhow!(
        "rule requires schema {required_schema}, but state is already at schema {}",
        self.inner.schema_version,
      ));
    }

    Ok(())
  }

  pub fn ensure_schema(&self, rule_version: &str, required_schema: u32) -> Result<()> {
    if self.inner.schema_version != required_schema {
      return Err(anyhow!(
        "rule {rule_version} expects state schema {required_schema}, got {}",
        self.inner.schema_version,
      ));
    }

    Ok(())
  }

  pub fn record_upgrade(&mut self) {
    self.inner.upgrades += 1;
  }

  pub fn snapshot(&self, current_rule_version: &str) -> ServiceSnapshot {
    let average_score = self.average_score();

    if let Some(v2) = &self.inner.v2 {
      return ServiceSnapshot::V2(ServiceSnapshotV2 {
        processed: self.inner.processed,
        schema_version: self.inner.schema_version,
        allow_count: self.inner.allow_count,
        review_count: self.inner.review_count,
        fast_lane_hits: self.inner.fast_lane_hits,
        upgrades: self.inner.upgrades,
        last_score: self.inner.last_score,
        average_score,
        current_rule_version: current_rule_version.to_owned(),
        migration_generation: v2.migration_generation,
        legacy_processed_at_migration: v2.legacy_processed_at_migration,
        fast_lane_amount: v2.fast_lane_amount,
        reviewed_amount: v2.reviewed_amount,
        largest_amount: v2.largest_amount,
        high_risk_requests: v2.high_risk_requests,
        late_night_reviews: v2.late_night_reviews,
        review_rate_bps: rate_bps(self.inner.review_count, self.inner.processed),
        fast_lane_rate_bps: rate_bps(self.inner.fast_lane_hits, self.inner.processed),
        last_decision: v2.last_decision.clone(),
        last_policy_id: v2.last_policy_id,
      });
    }

    ServiceSnapshot::V1(ServiceSnapshotV1 {
      processed: self.inner.processed,
      schema_version: self.inner.schema_version,
      allow_count: self.inner.allow_count,
      review_count: self.inner.review_count,
      upgrades: self.inner.upgrades,
      last_score: self.inner.last_score,
      average_score,
      current_rule_version: current_rule_version.to_owned(),
    })
  }

  fn average_score(&self) -> i32 {
    if self.inner.processed == 0 {
      0
    } else {
      (self.inner.total_score / self.inner.processed as i64) as i32
    }
  }

  pub(crate) fn to_component_state(&self) -> Result<component_rule::ServiceState> {
    state_to_component(&self.inner)
  }

  pub(crate) fn save_component_state(&mut self, state: component_rule::ServiceState) -> Result<()> {
    self.inner = state_from_component(&state)?;
    Ok(())
  }
}

pub(crate) fn state_to_component(state: &State) -> Result<component_rule::ServiceState> {
  Ok(component_rule::ServiceState {
    path: SERVICE_STATE_PATH.to_owned(),
    content_json: serde_json::to_string(state)
      .context("failed to serialize service state for wasm rule")?,
  })
}

pub(crate) fn state_from_component(state: &component_rule::ServiceState) -> Result<State> {
  if state.path != SERVICE_STATE_PATH {
    bail!(
      "wasm returned service state for unexpected path `{}`",
      state.path
    );
  }

  serde_json::from_str(&state.content_json)
    .with_context(|| format!("failed to decode service state at `{}`", state.path))
}

fn rate_bps(count: u64, total: u64) -> u32 {
  if total == 0 {
    0
  } else {
    ((count.saturating_mul(10_000)) / total) as u32
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn migrates_to_v1_snapshot_without_v2_stats() -> Result<()> {
    let mut state = ServiceState::default();
    state.migrate_to_schema(1)?;

    let snapshot = state.snapshot("risk_rule");
    assert_eq!(
      snapshot,
      ServiceSnapshot::V1(ServiceSnapshotV1 {
        processed: 0,
        schema_version: 1,
        allow_count: 0,
        review_count: 0,
        upgrades: 0,
        last_score: 0,
        average_score: 0,
        current_rule_version: "risk_rule".to_owned(),
      })
    );

    Ok(())
  }

  #[test]
  fn v2_state_records_extended_stats_after_migration() -> Result<()> {
    let mut state = ServiceState::default();
    state.migrate_to_schema(1)?;

    let mut payload = state_from_component(&state.to_component_state()?)?;
    payload.processed = 1;
    payload.allow_count = 1;
    payload.last_score = 30;
    payload.total_score = 30;
    state.save_component_state(state_to_component(&payload)?)?;

    state.record_upgrade();
    state.migrate_to_schema(2)?;

    let mut payload = state_from_component(&state.to_component_state()?)?;
    payload.processed = 2;
    payload.review_count = 1;
    payload.last_score = 88;
    payload.total_score = 118;
    payload.v2 = payload.v2.map(|mut v2| {
      v2.reviewed_amount = 4_800;
      v2.largest_amount = 4_800;
      v2.high_risk_requests = 1;
      v2.late_night_reviews = 1;
      v2.last_decision = Decision::Review;
      v2.last_policy_id = 202;
      v2
    });
    state.save_component_state(state_to_component(&payload)?)?;

    let snapshot = state.snapshot("risk_rule_v2");
    assert_eq!(
      snapshot,
      ServiceSnapshot::V2(ServiceSnapshotV2 {
        processed: 2,
        schema_version: 2,
        allow_count: 1,
        review_count: 1,
        fast_lane_hits: 0,
        upgrades: 1,
        last_score: 88,
        average_score: 59,
        current_rule_version: "risk_rule_v2".to_owned(),
        migration_generation: 2,
        legacy_processed_at_migration: 1,
        fast_lane_amount: 0,
        reviewed_amount: 4_800,
        largest_amount: 4_800,
        high_risk_requests: 1,
        late_night_reviews: 1,
        review_rate_bps: 5_000,
        fast_lane_rate_bps: 0,
        last_decision: Decision::Review,
        last_policy_id: 202,
      })
    );

    Ok(())
  }

  #[test]
  fn rejects_rule_that_requires_older_schema() -> Result<()> {
    let mut state = ServiceState::default();
    state.migrate_to_schema(2)?;

    let err = state.migrate_to_schema(1).unwrap_err();
    assert!(
      err
        .to_string()
        .contains("rule requires schema 1, but state is already at schema 2")
    );

    Ok(())
  }
}

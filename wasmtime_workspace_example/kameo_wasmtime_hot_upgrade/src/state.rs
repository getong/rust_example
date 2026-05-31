use anyhow::{Result, anyhow};

use crate::{bindings::exports::rule as component_rule, types::Decision};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct State {
  processed: u64,
  schema_version: u32,
  allow_count: u64,
  review_count: u64,
  fast_lane_hits: u64,
  upgrades: u64,
  last_score: i32,
  total_score: i64,
  v2: Option<StateV2Stats>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateV2Stats {
  migration_generation: u64,
  legacy_processed_at_migration: u64,
  fast_lane_amount: i64,
  reviewed_amount: i64,
  largest_amount: i64,
  high_risk_requests: u64,
  late_night_reviews: u64,
  last_decision: Decision,
  last_policy_id: i32,
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

  pub(crate) fn to_component_state(&self) -> component_rule::ServiceState {
    component_rule::ServiceState {
      processed: self.inner.processed,
      schema_version: self.inner.schema_version,
      allow_count: self.inner.allow_count,
      review_count: self.inner.review_count,
      fast_lane_hits: self.inner.fast_lane_hits,
      upgrades: self.inner.upgrades,
      last_score: self.inner.last_score,
      total_score: self.inner.total_score,
      v2: self.inner.v2.as_ref().map(StateV2Stats::to_component_state),
    }
  }

  pub(crate) fn save_component_state(&mut self, state: component_rule::ServiceState) {
    self.inner = State {
      processed: state.processed,
      schema_version: state.schema_version,
      allow_count: state.allow_count,
      review_count: state.review_count,
      fast_lane_hits: state.fast_lane_hits,
      upgrades: state.upgrades,
      last_score: state.last_score,
      total_score: state.total_score,
      v2: state.v2.map(StateV2Stats::from_component_state),
    };
  }
}

impl StateV2Stats {
  fn to_component_state(&self) -> component_rule::StateV2Stats {
    component_rule::StateV2Stats {
      migration_generation: self.migration_generation,
      legacy_processed_at_migration: self.legacy_processed_at_migration,
      fast_lane_amount: self.fast_lane_amount,
      reviewed_amount: self.reviewed_amount,
      largest_amount: self.largest_amount,
      high_risk_requests: self.high_risk_requests,
      late_night_reviews: self.late_night_reviews,
      last_decision: decision_to_component(&self.last_decision),
      last_policy_id: self.last_policy_id,
    }
  }

  fn from_component_state(state: component_rule::StateV2Stats) -> Self {
    Self {
      migration_generation: state.migration_generation,
      legacy_processed_at_migration: state.legacy_processed_at_migration,
      fast_lane_amount: state.fast_lane_amount,
      reviewed_amount: state.reviewed_amount,
      largest_amount: state.largest_amount,
      high_risk_requests: state.high_risk_requests,
      late_night_reviews: state.late_night_reviews,
      last_decision: state.last_decision.into(),
      last_policy_id: state.last_policy_id,
    }
  }
}

fn decision_to_component(decision: &Decision) -> component_rule::Decision {
  match decision {
    Decision::Allow => component_rule::Decision::Allow,
    Decision::Review => component_rule::Decision::Review,
    Decision::AllowFastLane => component_rule::Decision::AllowFastLane,
  }
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

    let mut component_state = state.to_component_state();
    component_state.processed = 1;
    component_state.allow_count = 1;
    component_state.last_score = 30;
    component_state.total_score = 30;
    state.save_component_state(component_state);

    state.record_upgrade();
    state.migrate_to_schema(2)?;

    let mut component_state = state.to_component_state();
    component_state.processed = 2;
    component_state.review_count = 1;
    component_state.last_score = 88;
    component_state.total_score = 118;
    component_state.v2 = component_state.v2.map(|mut v2| {
      v2.reviewed_amount = 4_800;
      v2.largest_amount = 4_800;
      v2.high_risk_requests = 1;
      v2.late_night_reviews = 1;
      v2.last_decision = component_rule::Decision::Review;
      v2.last_policy_id = 202;
      v2
    });
    state.save_component_state(component_state);

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

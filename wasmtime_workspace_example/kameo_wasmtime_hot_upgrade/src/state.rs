use anyhow::{Result, anyhow};

use crate::types::{Decision, Request, Response};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct State {
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
struct StateV2Stats {
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

  pub fn record_response(&mut self, request: &Request, response: &Response) {
    self.inner.processed += 1;
    self.inner.last_score = response.risk_score;
    self.inner.total_score += i64::from(response.risk_score);

    match &response.decision {
      Decision::Allow => self.inner.allow_count += 1,
      Decision::AllowFastLane => {
        self.inner.allow_count += 1;
        self.inner.fast_lane_hits += 1;
      }
      Decision::Review => self.inner.review_count += 1,
    }

    if let Some(v2) = &mut self.inner.v2 {
      record_v2_stats(v2, request, response);
    }
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
}

fn record_v2_stats(v2: &mut StateV2Stats, request: &Request, response: &Response) {
  v2.last_decision = response.decision.clone();
  v2.last_policy_id = response.policy_id;
  v2.largest_amount = v2.largest_amount.max(request.amount);

  if request.merchant_risk >= 80 {
    v2.high_risk_requests += 1;
  }

  match &response.decision {
    Decision::AllowFastLane => v2.fast_lane_amount += request.amount,
    Decision::Review => {
      v2.reviewed_amount += request.amount;
      if request.hour <= 5 {
        v2.late_night_reviews += 1;
      }
    }
    Decision::Allow => {}
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
    state.record_response(
      &Request {
        user_id: 11,
        amount: 6_000,
        merchant_risk: 20,
        hour: 14,
      },
      &Response {
        decision: Decision::Allow,
        rule_version: "risk_rule".to_owned(),
        policy_id: 101,
        risk_score: 30,
      },
    );
    state.record_upgrade();
    state.migrate_to_schema(2)?;
    state.record_response(
      &Request {
        user_id: 37,
        amount: 4_800,
        merchant_risk: 86,
        hour: 2,
      },
      &Response {
        decision: Decision::Review,
        rule_version: "risk_rule_v2".to_owned(),
        policy_id: 202,
        risk_score: 88,
      },
    );

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

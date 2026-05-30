use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
  pub user_id: i64,
  pub amount: i64,
  pub merchant_risk: i32,
  pub hour: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
  Allow,
  Review,
  AllowFastLane,
}

impl Decision {
  pub fn as_str(&self) -> &'static str {
    match self {
      Self::Allow => "allow",
      Self::Review => "review",
      Self::AllowFastLane => "allow-fast-lane",
    }
  }
}

impl fmt::Display for Decision {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.as_str())
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
  pub decision: Decision,
  pub rule_version: String,
  pub policy_id: i32,
  pub risk_score: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
  pub processed: u64,
  pub schema_version: u32,
  pub allow_count: u64,
  pub review_count: u64,
  pub fast_lane_hits: u64,
  pub upgrades: u64,
  pub last_score: i32,
  pub total_score: i64,
  pub v2: Option<StateV2Stats>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateV2Stats {
  pub migration_generation: u64,
  pub legacy_processed_at_migration: u64,
  pub fast_lane_amount: i64,
  pub reviewed_amount: i64,
  pub largest_amount: i64,
  pub high_risk_requests: u64,
  pub late_night_reviews: u64,
  pub last_decision: Decision,
  pub last_policy_id: i32,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleMetadata {
  pub version: String,
  pub required_schema: u32,
  pub policy_id: i32,
  pub dependency_marker: i32,
  pub review_threshold: i32,
  pub fast_lane_limit: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleInspection {
  pub metadata: RuleMetadata,
  pub sample_request: Request,
  pub sample_score: i32,
}

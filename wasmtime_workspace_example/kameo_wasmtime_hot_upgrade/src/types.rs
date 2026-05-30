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
  pub runtime: RuleRuntimeSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleRuntimeSnapshot {
  pub version: String,
  pub component_path: String,
  pub loaded_required_schema: u32,
  pub metadata_calls: u64,
  pub evaluate_calls: u64,
  pub last_request: Option<Request>,
  pub last_response: Option<Response>,
}

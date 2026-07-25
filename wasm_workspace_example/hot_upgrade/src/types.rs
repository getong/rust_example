use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
  pub user_id: i64,
  pub amount: i64,
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
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
  pub processed: u64,
  pub schema_version: u32,
  pub fast_lane_hits: u64,
  pub upgrades: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceSnapshot {
  pub processed: u64,
  pub schema_version: u32,
  pub fast_lane_hits: u64,
  pub upgrades: u64,
  pub current_rule_version: String,
}

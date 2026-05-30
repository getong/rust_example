#![no_std]

extern crate alloc;

use kameo_risk_rule_support as _;

wit_bindgen::generate!({
  path: "../wit/risk-rule.wit",
  world: "risk-rule",
});

const POLICY_ID: i32 = 101;
const REQUIRED_SCHEMA: i32 = 1;
const REVIEW_THRESHOLD: i32 = 75;
const FAST_LANE_LIMIT: i64 = 0;
const FINGERPRINT_WEIGHT: i32 = 4;

struct RiskRule;

#[derive(Clone, Copy)]
struct Transaction {
  user_id: i64,
  amount: i64,
  merchant_risk: i32,
  hour: i32,
}

impl Transaction {
  fn new(user_id: i64, amount: i64, merchant_risk: i32, hour: i32) -> Self {
    Self {
      user_id,
      amount,
      merchant_risk,
      hour,
    }
  }

  fn amount_band(self) -> i32 {
    match self.amount {
      amount if amount >= 10_000 => 55,
      amount if amount >= 7_000 => 35,
      amount if amount >= 5_000 => 22,
      amount if amount >= 2_000 => 10,
      _ => 3,
    }
  }

  fn merchant_band(self) -> i32 {
    (self.merchant_risk / 3).clamp(0, 30)
  }

  fn hour_band(self) -> i32 {
    if self.hour <= 5 { 8 } else { 0 }
  }

  fn known_user_discount(self) -> i32 {
    if self.user_id % 5 == 0 { 6 } else { 0 }
  }

  fn fingerprint(self) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    let mut buffer = itoa::Buffer::new();

    hasher.update(buffer.format(self.user_id).as_bytes());
    hasher.update(b":");
    hasher.update(buffer.format(self.amount).as_bytes());
    hasher.update(b":");
    hasher.update(buffer.format(self.merchant_risk).as_bytes());
    hasher.update(b":");
    hasher.update(buffer.format(self.hour).as_bytes());

    hasher.finalize()
  }

  fn dependency_adjustment(self) -> i32 {
    let fingerprint_band = (self.fingerprint() % FINGERPRINT_WEIGHT as u32) as i32;
    let smoothed_risk = libm::sqrt(self.merchant_risk.max(0) as f64) as i32;
    fingerprint_band + smoothed_risk / 3
  }
}

fn dependency_marker() -> i32 {
  Transaction::new(101, 6_000, 20, 14).dependency_adjustment()
}

fn risk_score(request: exports::rule::Request) -> i32 {
  let tx = Transaction::new(
    request.user_id,
    request.amount,
    request.merchant_risk,
    request.hour,
  );
  let score = tx.amount_band() + tx.merchant_band() + tx.hour_band() + tx.dependency_adjustment()
    - tx.known_user_discount();
  score.clamp(0, 100)
}

fn evaluate(request: exports::rule::Request) -> exports::rule::Evaluation {
  host::method_enter("evaluate", POLICY_ID);
  let required_schema = host::loaded_required_schema();
  if required_schema != REQUIRED_SCHEMA as u32 {
    host::record_last_score(100);
    return exports::rule::Evaluation {
      decision: exports::rule::Decision::Review,
      risk_score: 100,
      policy_id: POLICY_ID,
    };
  }

  let risk_score = risk_score(request);
  let decision = if risk_score >= REVIEW_THRESHOLD {
    exports::rule::Decision::Review
  } else {
    exports::rule::Decision::Allow
  };

  host::record_last_score(risk_score);
  exports::rule::Evaluation {
    decision,
    risk_score,
    policy_id: POLICY_ID,
  }
}

impl exports::rule::Guest for RiskRule {
  fn metadata() -> exports::rule::RuleMetadata {
    exports::rule::RuleMetadata {
      required_schema: REQUIRED_SCHEMA as u32,
      policy_id: POLICY_ID,
      dependency_marker: dependency_marker(),
      review_threshold: REVIEW_THRESHOLD,
      fast_lane_limit: FAST_LANE_LIMIT,
    }
  }

  fn evaluate(request: exports::rule::Request) -> exports::rule::Evaluation {
    evaluate(request)
  }
}

export!(RiskRule);

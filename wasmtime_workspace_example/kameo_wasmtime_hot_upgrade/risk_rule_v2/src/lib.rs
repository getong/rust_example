#![no_std]

extern crate alloc;

use kameo_risk_rule_support as _;

wit_bindgen::generate!({
  path: "../wit/risk-rule.wit",
  world: "risk-rule",
});

const POLICY_ID: i32 = 202;
const REQUIRED_SCHEMA: i32 = 2;
const REVIEW_THRESHOLD: i32 = 65;
const FAST_LANE_LIMIT: i64 = 4_000;
const TRUSTED_MERCHANT_MAX_RISK: i32 = 15;
const FINGERPRINT_WEIGHT: i32 = 7;

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
      amount if amount >= 8_000 => 55,
      amount if amount >= 5_000 => 38,
      amount if amount >= 3_000 => 18,
      amount if amount >= 1_000 => 6,
      _ => 2,
    }
  }

  fn merchant_band(self) -> i32 {
    (self.merchant_risk / 2).clamp(0, 45)
  }

  fn hour_band(self) -> i32 {
    match self.hour {
      0 ..= 4 => 18,
      5 ..= 6 => 8,
      _ => 0,
    }
  }

  fn trusted_user_discount(self) -> i32 {
    if self.user_id % 2 == 0 { 12 } else { 0 }
  }

  fn qualifies_for_fast_lane(self) -> bool {
    self.user_id % 2 == 0
      && self.amount <= FAST_LANE_LIMIT
      && self.merchant_risk <= TRUSTED_MERCHANT_MAX_RISK
      && self.hour >= 7
  }

  fn fingerprint(self) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    let mut buffer = itoa::Buffer::new();

    hasher.update(b"v2:");
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
    let merchant_curve = libm::log10((self.merchant_risk.max(0) + 10) as f64);
    fingerprint_band + (merchant_curve * 5.0) as i32
  }
}

fn dependency_marker() -> i32 {
  Transaction::new(202, 6_000, 20, 14).dependency_adjustment()
}

fn risk_score(request: exports::rule::Request) -> i32 {
  let tx = Transaction::new(
    request.user_id,
    request.amount,
    request.merchant_risk,
    request.hour,
  );
  let score = tx.amount_band() + tx.merchant_band() + tx.hour_band() + tx.dependency_adjustment()
    - tx.trusted_user_discount();
  score.clamp(0, 100)
}

fn evaluate(request: exports::rule::Request) -> exports::rule::Evaluation {
  let tx = Transaction::new(
    request.user_id,
    request.amount,
    request.merchant_risk,
    request.hour,
  );
  let risk_score = risk_score(request);
  let decision = if tx.qualifies_for_fast_lane() {
    exports::rule::Decision::AllowFastLane
  } else if risk_score >= REVIEW_THRESHOLD {
    exports::rule::Decision::Review
  } else {
    exports::rule::Decision::Allow
  };

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

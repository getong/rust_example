wit_bindgen::generate!({
  inline: r#"
package kameo:risk@0.1.0;

world risk-rule {
    import host: interface {
        method-enter: func(method: string, policy-id: s32);
        loaded-required-schema: func() -> u32;
        evaluate-call-count: func() -> u64;
        record-last-score: func(score: s32);
    }

    export rule: interface {
        record request {
            user-id: s64,
            amount: s64,
            merchant-risk: s32,
            hour: s32,
        }

        enum decision {
            allow,
            review,
            allow-fast-lane,
        }

        record service-state {
            path: string,
            content-json: string,
        }

        record evaluation {
            decision: decision,
            risk-score: s32,
            policy-id: s32,
        }

        record rule-metadata {
            required-schema: u32,
            policy-id: s32,
            dependency-marker: s32,
            review-threshold: s32,
            fast-lane-limit: s64,
        }

        record evaluate-result {
            evaluation: evaluation,
            state: service-state,
        }

        metadata: func() -> rule-metadata;
        evaluate: func(request: request, state: service-state) -> evaluate-result;
    }
}
"#,
  world: "risk-rule",
});

use serde::{Deserialize, Serialize};

const POLICY_ID: i32 = 101;
const REQUIRED_SCHEMA: i32 = 1;
const REVIEW_THRESHOLD: i32 = 75;
const FAST_LANE_LIMIT: i64 = 0;
const FINGERPRINT_WEIGHT: i32 = 4;
const SERVICE_STATE_PATH: &str = "state/service.json";

struct RiskRule;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ServiceDecision {
  Allow,
  Review,
  AllowFastLane,
}

#[derive(Clone, Serialize, Deserialize)]
struct ServiceStateV2Stats {
  migration_generation: u64,
  legacy_processed_at_migration: u64,
  fast_lane_amount: i64,
  reviewed_amount: i64,
  largest_amount: i64,
  high_risk_requests: u64,
  late_night_reviews: u64,
  last_decision: ServiceDecision,
  last_policy_id: i32,
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct ServiceStatePayload {
  processed: u64,
  schema_version: u32,
  allow_count: u64,
  review_count: u64,
  fast_lane_hits: u64,
  upgrades: u64,
  last_score: i32,
  total_score: i64,
  v2: Option<ServiceStateV2Stats>,
}

impl From<exports::rule::Decision> for ServiceDecision {
  fn from(decision: exports::rule::Decision) -> Self {
    match decision {
      exports::rule::Decision::Allow => Self::Allow,
      exports::rule::Decision::Review => Self::Review,
      exports::rule::Decision::AllowFastLane => Self::AllowFastLane,
    }
  }
}

fn decode_state(state: &exports::rule::ServiceState) -> ServiceStatePayload {
  if state.path != SERVICE_STATE_PATH {
    return ServiceStatePayload::default();
  }

  serde_json::from_str(&state.content_json).unwrap_or_default()
}

fn encode_state(payload: &ServiceStatePayload) -> exports::rule::ServiceState {
  exports::rule::ServiceState {
    path: SERVICE_STATE_PATH.to_owned(),
    content_json: serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_owned()),
  }
}

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

fn record_response(
  state: &mut ServiceStatePayload,
  request: exports::rule::Request,
  evaluation: &exports::rule::Evaluation,
) {
  state.processed += 1;
  state.last_score = evaluation.risk_score;
  state.total_score += i64::from(evaluation.risk_score);

  match evaluation.decision {
    exports::rule::Decision::Allow => state.allow_count += 1,
    exports::rule::Decision::AllowFastLane => {
      state.allow_count += 1;
      state.fast_lane_hits += 1;
    }
    exports::rule::Decision::Review => state.review_count += 1,
  }

  if let Some(v2) = &mut state.v2 {
    v2.last_decision = evaluation.decision.into();
    v2.last_policy_id = evaluation.policy_id;
    v2.largest_amount = v2.largest_amount.max(request.amount);

    if request.merchant_risk >= 80 {
      v2.high_risk_requests += 1;
    }

    match evaluation.decision {
      exports::rule::Decision::AllowFastLane => v2.fast_lane_amount += request.amount,
      exports::rule::Decision::Review => {
        v2.reviewed_amount += request.amount;
        if request.hour <= 5 {
          v2.late_night_reviews += 1;
        }
      }
      exports::rule::Decision::Allow => {}
    }
  }
}

fn evaluate(
  request: exports::rule::Request,
  state: exports::rule::ServiceState,
) -> exports::rule::EvaluateResult {
  host::method_enter("evaluate", POLICY_ID);
  let mut payload = decode_state(&state);
  let required_schema = host::loaded_required_schema();
  let evaluation = if required_schema != REQUIRED_SCHEMA as u32 {
    exports::rule::Evaluation {
      decision: exports::rule::Decision::Review,
      risk_score: 100,
      policy_id: POLICY_ID,
    }
  } else {
    let risk_score = risk_score(request);
    let decision = if risk_score >= REVIEW_THRESHOLD {
      exports::rule::Decision::Review
    } else {
      exports::rule::Decision::Allow
    };

    exports::rule::Evaluation {
      decision,
      risk_score,
      policy_id: POLICY_ID,
    }
  };

  host::record_last_score(evaluation.risk_score);
  record_response(&mut payload, request, &evaluation);

  exports::rule::EvaluateResult {
    evaluation,
    state: encode_state(&payload),
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

  fn evaluate(
    request: exports::rule::Request,
    state: exports::rule::ServiceState,
  ) -> exports::rule::EvaluateResult {
    evaluate(request, state)
  }
}

export!(RiskRule);

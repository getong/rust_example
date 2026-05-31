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

        record state-v2-stats {
            migration-generation: u64,
            legacy-processed-at-migration: u64,
            fast-lane-amount: s64,
            reviewed-amount: s64,
            largest-amount: s64,
            high-risk-requests: u64,
            late-night-reviews: u64,
            last-decision: decision,
            last-policy-id: s32,
        }

        record service-state {
            processed: u64,
            schema-version: u32,
            allow-count: u64,
            review-count: u64,
            fast-lane-hits: u64,
            upgrades: u64,
            last-score: s32,
            total-score: s64,
            v2: option<state-v2-stats>,
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

fn record_response(
  state: &mut exports::rule::ServiceState,
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
    v2.last_decision = evaluation.decision;
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
  mut state: exports::rule::ServiceState,
) -> exports::rule::EvaluateResult {
  host::method_enter("evaluate", POLICY_ID);
  let required_schema = host::loaded_required_schema();
  let evaluation = if required_schema != REQUIRED_SCHEMA as u32 {
    exports::rule::Evaluation {
      decision: exports::rule::Decision::Review,
      risk_score: 100,
      policy_id: POLICY_ID,
    }
  } else {
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
  };

  host::record_last_score(evaluation.risk_score);
  record_response(&mut state, request, &evaluation);

  exports::rule::EvaluateResult { evaluation, state }
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

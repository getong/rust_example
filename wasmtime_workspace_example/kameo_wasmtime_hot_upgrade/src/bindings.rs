#![allow(clippy::all)]

wasmtime::component::bindgen!({
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
  with: {},
  require_store_data_send: true,
});

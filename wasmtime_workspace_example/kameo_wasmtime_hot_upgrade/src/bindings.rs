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

        metadata: func() -> rule-metadata;
        evaluate: func(request: request) -> evaluation;
    }
}
"#,
  world: "risk-rule",
  with: {},
  require_store_data_send: true,
});

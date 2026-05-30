# Kameo + Wasmtime Hot Upgrade

This example combines the two existing examples in this workspace:

- `hot_upgrade`: Wasmtime loads a risk-rule module, validates it against shadow state, migrates state, and swaps the active rule.
- `kameo_custom_swarm`: application logic is expressed as a kameo actor receiving typed messages.

The host keeps long-lived service state inside `HotUpgradeActor`. Calls, rule inspection, snapshots, and upgrades are kameo messages. Because kameo processes normal messages for an actor sequentially, a rule upgrade is committed between requests: the actor loads the next `.wasm`, dry-runs it on cloned state, migrates the real state, and only then swaps the active rule.

The WASM rules use the Component Model instead of hand-written exported
symbols. The ABI contract lives in `wit/risk-rule.wit`; each rule crate uses
`wit_bindgen::generate!` plus `export!(RiskRule)` to implement the generated
`exports::rule::Guest` trait. The host uses `wasmtime::component::bindgen!`
and calls the generated `rule().call_*` methods, so it never looks up raw
function names and the rule crates do not use `#[no_mangle]`.

The WIT interface exports:

- `metadata() -> rule-metadata`
- `risk-score(request) -> s32`
- `decide(request) -> decision`

`risk_rule_v1` is intentionally conservative and only returns `allow` or `review`. `risk_rule_v2` changes the scoring model, lowers the review threshold, and adds `allow-fast-lane` for trusted even-numbered users with low-risk small transactions.

Both rule crates intentionally call third-party Rust crates inside the WASM module:

- `crc32fast`: creates a stable transaction fingerprint.
- `itoa`: formats numeric fields into the fingerprint input without allocation-heavy formatting.
- `libm`: applies a deterministic math curve to merchant risk.

The host prints `deps_marker`, which is calculated inside the currently loaded WASM module using those dependencies. After the hot upgrade, that marker and the risk scores change without restarting the kameo actor.

## Run

Build the two rule modules first:

```sh
cd kameo_wasmtime_hot_upgrade
rustup target add wasm32-wasip2

cargo build --package kameo_risk_rule_v1 --release --target wasm32-wasip2
mkdir -p rules/current
cp target/wasm32-wasip2/release/kameo_risk_rule_v1.wasm \
  rules/current/risk_rule.wasm

cargo build --package kameo_risk_rule_v2 --release --target wasm32-wasip2
mkdir -p rules/releases
cp target/wasm32-wasip2/release/kameo_risk_rule_v2.wasm \
  rules/releases/risk_rule_v2.wasm
```

Then run the demo:

```sh
cargo run --package kameo_wasmtime_hot_upgrade
```

Expected output shape:

```text
--- boot with v1 wasm rule ---
active_rule=risk_rule, schema=1, policy=101, deps_marker=3, threshold=75, fast_lane_limit=0, sample_score=29
mid amount, standard merchant      user=11  amount=6000  merchant_risk=20  hour=14 => policy=101 score=29  risk_rule:allow
trusted even user, small amount    user=22  amount=2500  merchant_risk=8   hour=10 => policy=101 score=15  risk_rule:allow
late-night risky merchant          user=37  amount=4800  merchant_risk=86  hour=2  => policy=101 score=51  risk_rule:allow
snapshot before upgrade: processed=3, allow=3, review=0, fast_lane=0, schema=1, current_rule=risk_rule, upgrades=0, avg_score=31, last_score=51

--- hot upgrade: load v2 wasm rule ---
upgrade risk_rule -> risk_rule_v2

--- same requests after v2 takes over ---
active_rule=risk_rule_v2, schema=2, policy=202, deps_marker=7, threshold=65, fast_lane_limit=4000, sample_score=58
mid amount, standard merchant      user=11  amount=6000  merchant_risk=20  hour=14 => policy=202 score=58  risk_rule_v2:allow
trusted even user, small amount    user=22  amount=2500  merchant_risk=8   hour=10 => policy=202 score=4   risk_rule_v2:allow-fast-lane
late-night risky merchant          user=37  amount=4800  merchant_risk=86  hour=2  => policy=202 score=88  risk_rule_v2:review
snapshot after upgrade: processed=6, allow=5, review=1, fast_lane=1, schema=2, current_rule=risk_rule_v2, upgrades=1, avg_score=40, last_score=88
```

The first module is copied to `rules/current/risk_rule.wasm`, so its runtime version is `risk_rule`. The second module keeps the release filename `risk_rule_v2.wasm`, making the swap visible in the output.

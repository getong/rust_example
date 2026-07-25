//! Demonstration of the WASM-based hot-upgrade service.
//!
//! # Prerequisites
//!
//! Build the WASM rule modules first:
//!
//! ```sh
//! rustup target add wasm32-unknown-unknown
//!
//! cargo build --package risk_rule_v1 --release --target wasm32-unknown-unknown
//! cp target/wasm32-unknown-unknown/release/risk_rule_v1.wasm rules/current/risk_rule.wasm
//!
//! cargo build --package risk_rule_v2 --release --target wasm32-unknown-unknown
//! cp target/wasm32-unknown-unknown/release/risk_rule_v2.wasm rules/releases/risk_rule_v2.wasm
//! ```
//!
//! Then run:
//!
//! ```sh
//! cargo run --bin wasm_demo
//! ```

use std::path::PathBuf;

use anyhow::Result;
use hot_upgrade::{Request, wasm_service::start_wasm_service};

fn rules_dir() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules")
}

#[tokio::main]
async fn main() -> Result<()> {
  // -----------------------------------------------------------------------
  // T0: Service starts – only the current (v1) wasm rule is known.
  // -----------------------------------------------------------------------
  let service = start_wasm_service(rules_dir().join("current/risk_rule.wasm"))?;
  let handle = service.handle();

  println!("--- v1 rule active ---");
  for request in [
    Request {
      user_id: 1,
      amount: 6_000,
    },
    Request {
      user_id: 9,
      amount: 12_000,
    },
  ] {
    let response = handle.call(request).await?;
    println!("{}:{}", response.rule_version, response.decision);
  }

  // -----------------------------------------------------------------------
  // T2-T3: Management plane publishes v2 and sends an Upgrade message.
  //
  // The Actor processes any in-flight requests first, then:
  //   1. Loads the new wasm module.
  //   2. Validates it against a shadow state copy.
  //   3. Migrates real state to the new schema.
  //   4. Swaps the handler atomically.
  // -----------------------------------------------------------------------
  let message = handle
    .upgrade(rules_dir().join("releases/risk_rule_v2.wasm"))
    .await?;
  println!("\n{message}\n");

  // -----------------------------------------------------------------------
  // T4: New requests are handled by the v2 rule; state was preserved.
  // -----------------------------------------------------------------------
  println!("--- v2 rule active ---");
  for request in [
    Request {
      user_id: 2,
      amount: 3_000,
    },
    Request {
      user_id: 3,
      amount: 6_000,
    },
  ] {
    let response = handle.call(request).await?;
    println!("{}:{}", response.rule_version, response.decision);
  }

  let snapshot = handle.snapshot().await?;
  println!(
    "\nprocessed={}, schema={}, fast_lane_hits={}, current_rule={}, upgrades={}",
    snapshot.processed,
    snapshot.schema_version,
    snapshot.fast_lane_hits,
    snapshot.current_rule_version,
    snapshot.upgrades,
  );

  service.shutdown().await?;
  Ok(())
}

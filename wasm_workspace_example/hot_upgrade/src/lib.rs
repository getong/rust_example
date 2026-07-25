pub mod rule;
pub mod service;
pub mod types;
pub mod wasm_rule;
pub mod wasm_service;

pub use service::{HotServiceHandle, StartedService, start_service};
pub use types::{Decision, Request, Response, ServiceSnapshot, State};
pub use wasm_service::{WasmServiceHandle, WasmStartedService, start_wasm_service};

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use anyhow::Result;

  use super::*;

  fn rules_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules")
  }

  #[tokio::test]
  async fn state_survives_upgrade_and_new_rule_takes_over() -> Result<()> {
    let service = start_service(rules_dir().join("current/risk_rule.json"))?;
    let handle = service.handle();

    let first = handle
      .call(Request {
        user_id: 1,
        amount: 6_000,
      })
      .await?;
    assert_eq!(first.rule_version, "risk_rule_v1");
    assert_eq!(first.decision, Decision::Allow);

    let second = handle
      .call(Request {
        user_id: 9,
        amount: 12_000,
      })
      .await?;
    assert_eq!(second.decision, Decision::Review);

    let message = handle
      .upgrade(rules_dir().join("releases/risk_rule_v2.json"))
      .await?;
    assert_eq!(message, "upgrade risk_rule_v1 -> risk_rule_v2");

    let third = handle
      .call(Request {
        user_id: 2,
        amount: 3_000,
      })
      .await?;
    assert_eq!(third.rule_version, "risk_rule_v2");
    assert_eq!(third.decision, Decision::AllowFastLane);

    let snapshot = handle.snapshot().await?;
    assert_eq!(snapshot.processed, 3);
    assert_eq!(snapshot.schema_version, 2);
    assert_eq!(snapshot.fast_lane_hits, 1);
    assert_eq!(snapshot.upgrades, 1);
    assert_eq!(snapshot.current_rule_version, "risk_rule_v2");

    service.shutdown().await?;
    Ok(())
  }
}

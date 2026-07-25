use std::path::PathBuf;

use anyhow::Result;
use hot_upgrade::{Request, start_service};

fn rules_dir() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules")
}

#[tokio::main]
async fn main() -> Result<()> {
  let service = start_service(rules_dir().join("current/risk_rule.json"))?;
  let handle = service.handle();

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

  let message = handle
    .upgrade(rules_dir().join("releases/risk_rule_v2.json"))
    .await?;
  println!("{message}");

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
    "processed={}, schema={}, fast_lane_hits={}, current_rule={}, upgrades={}",
    snapshot.processed,
    snapshot.schema_version,
    snapshot.fast_lane_hits,
    snapshot.current_rule_version,
    snapshot.upgrades
  );

  service.shutdown().await?;
  Ok(())
}

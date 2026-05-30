use std::path::PathBuf;

use anyhow::Result;
use kameo::prelude::*;
use kameo_wasmtime_hot_upgrade::{
  CallRule, HotUpgradeActor, InspectRule, Request, Response, Snapshot, UpgradeRule,
  start_hot_upgrade_actor,
};

struct Scenario {
  label: &'static str,
  request: Request,
}

pub async fn run() -> Result<()> {
  let service = start_hot_upgrade_actor(rules_dir().join("current/risk_rule.wasm"))?;
  let actor_ref = service.actor_ref();

  let scenarios = scenarios();

  println!("--- boot with v1 wasm rule ---");
  inspect_rule(&actor_ref, &scenarios[0].request).await?;
  submit_batch(&actor_ref, &scenarios).await?;
  print_snapshot(&actor_ref, "snapshot before upgrade").await?;

  println!("\n--- hot upgrade: load v2 wasm rule ---");
  let message = actor_ref
    .ask(UpgradeRule {
      wasm_path: rules_dir().join("releases/risk_rule_v2.wasm"),
    })
    .await?;
  println!("{message}");

  println!("\n--- same requests after v2 takes over ---");
  inspect_rule(&actor_ref, &scenarios[0].request).await?;
  submit_batch(&actor_ref, &scenarios).await?;
  print_snapshot(&actor_ref, "snapshot after upgrade").await?;

  service.shutdown().await?;
  Ok(())
}

fn rules_dir() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules")
}

fn scenarios() -> Vec<Scenario> {
  vec![
    Scenario {
      label: "mid amount, standard merchant",
      request: Request {
        user_id: 11,
        amount: 6_000,
        merchant_risk: 20,
        hour: 14,
      },
    },
    Scenario {
      label: "trusted even user, small amount",
      request: Request {
        user_id: 22,
        amount: 2_500,
        merchant_risk: 8,
        hour: 10,
      },
    },
    Scenario {
      label: "late-night risky merchant",
      request: Request {
        user_id: 37,
        amount: 4_800,
        merchant_risk: 86,
        hour: 2,
      },
    },
  ]
}

async fn inspect_rule(actor_ref: &ActorRef<HotUpgradeActor>, sample: &Request) -> Result<()> {
  let inspection = actor_ref
    .ask(InspectRule {
      sample: sample.clone(),
    })
    .await?;
  let metadata = inspection.metadata;

  println!(
    "active_rule={}, schema={}, policy={}, deps_marker={}, threshold={}, fast_lane_limit={}, \
     sample_score={}",
    metadata.version,
    metadata.required_schema,
    metadata.policy_id,
    metadata.dependency_marker,
    metadata.review_threshold,
    metadata.fast_lane_limit,
    inspection.sample_score,
  );

  Ok(())
}

async fn submit_batch(actor_ref: &ActorRef<HotUpgradeActor>, scenarios: &[Scenario]) -> Result<()> {
  for scenario in scenarios {
    let response = actor_ref.ask(CallRule(scenario.request.clone())).await?;
    print_response(scenario, &response);
  }
  Ok(())
}

fn print_response(scenario: &Scenario, response: &Response) {
  println!(
    "{:<34} user={:<3} amount={:<5} merchant_risk={:<3} hour={:<2} => policy={} score={:<3} {}:{}",
    scenario.label,
    scenario.request.user_id,
    scenario.request.amount,
    scenario.request.merchant_risk,
    scenario.request.hour,
    response.policy_id,
    response.risk_score,
    response.rule_version,
    response.decision,
  );
}

async fn print_snapshot(actor_ref: &ActorRef<HotUpgradeActor>, label: &str) -> Result<()> {
  let snapshot = actor_ref.ask(Snapshot).await?;
  println!(
    "{}: processed={}, allow={}, review={}, fast_lane={}, schema={}, current_rule={}, \
     upgrades={}, avg_score={}, last_score={}",
    label,
    snapshot.processed,
    snapshot.allow_count,
    snapshot.review_count,
    snapshot.fast_lane_hits,
    snapshot.schema_version,
    snapshot.current_rule_version,
    snapshot.upgrades,
    snapshot.average_score,
    snapshot.last_score,
  );
  Ok(())
}

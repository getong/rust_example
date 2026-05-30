use std::path::{Path, PathBuf};

use anyhow::{Context as AnyhowContext, Result, anyhow};
use kameo::prelude::*;
use wasmtime::{Config, Engine};

use crate::{
  types::{
    Request, Response, RuleInspection, ServiceSnapshot, ServiceSnapshotV1, ServiceSnapshotV2, State,
  },
  wasm_rule::WasmRule,
};

#[derive(Actor)]
pub struct HotUpgradeActor {
  engine: Engine,
  state: State,
  rule: WasmRule,
}

impl HotUpgradeActor {
  fn load(initial_rule: impl AsRef<Path>) -> Result<Self> {
    let initial_rule = initial_rule.as_ref();
    let engine = component_engine()?;
    let rule = WasmRule::load(&engine, initial_rule).with_context(|| {
      format!(
        "failed to load initial wasm rule {}",
        initial_rule.display()
      )
    })?;

    let mut state = State::default();
    rule.migrate_state(&mut state)?;

    Ok(Self {
      engine,
      state,
      rule,
    })
  }

  fn upgrade(&mut self, wasm_path: PathBuf) -> Result<String> {
    let old_version = self.rule.version().to_owned();
    let mut next = WasmRule::load(&self.engine, &wasm_path)?;

    self.validate(&mut next)?;
    next.migrate_state(&mut self.state)?;
    self.state.upgrades += 1;
    self.rule = next;

    Ok(format!("upgrade {old_version} -> {}", self.rule.version()))
  }

  fn validate(&self, next: &mut WasmRule) -> Result<()> {
    let mut shadow = self.state.clone();
    next.migrate_state(&mut shadow)?;

    let response = next.handle(
      &mut shadow,
      Request {
        user_id: 2,
        amount: 100,
        merchant_risk: 5,
        hour: 12,
      },
    )?;

    if response.rule_version.trim().is_empty() {
      return Err(anyhow!("new wasm rule returned an empty version"));
    }

    Ok(())
  }

  fn snapshot(&self) -> ServiceSnapshot {
    let average_score = if self.state.processed == 0 {
      0
    } else {
      (self.state.total_score / self.state.processed as i64) as i32
    };

    if let Some(v2) = &self.state.v2 {
      return ServiceSnapshot::V2(ServiceSnapshotV2 {
        processed: self.state.processed,
        schema_version: self.state.schema_version,
        allow_count: self.state.allow_count,
        review_count: self.state.review_count,
        fast_lane_hits: self.state.fast_lane_hits,
        upgrades: self.state.upgrades,
        last_score: self.state.last_score,
        average_score,
        current_rule_version: self.rule.version().to_owned(),
        migration_generation: v2.migration_generation,
        legacy_processed_at_migration: v2.legacy_processed_at_migration,
        fast_lane_amount: v2.fast_lane_amount,
        reviewed_amount: v2.reviewed_amount,
        largest_amount: v2.largest_amount,
        high_risk_requests: v2.high_risk_requests,
        late_night_reviews: v2.late_night_reviews,
        review_rate_bps: rate_bps(self.state.review_count, self.state.processed),
        fast_lane_rate_bps: rate_bps(self.state.fast_lane_hits, self.state.processed),
        last_decision: v2.last_decision.clone(),
        last_policy_id: v2.last_policy_id,
      });
    }

    ServiceSnapshot::V1(ServiceSnapshotV1 {
      processed: self.state.processed,
      schema_version: self.state.schema_version,
      allow_count: self.state.allow_count,
      review_count: self.state.review_count,
      upgrades: self.state.upgrades,
      last_score: self.state.last_score,
      average_score,
      current_rule_version: self.rule.version().to_owned(),
    })
  }
}

fn rate_bps(count: u64, total: u64) -> u32 {
  if total == 0 {
    0
  } else {
    ((count.saturating_mul(10_000)) / total) as u32
  }
}

fn component_engine() -> Result<Engine> {
  let mut config = Config::new();
  config.wasm_component_model(true);
  Engine::new(&config).map_err(Into::into)
}

pub struct CallRule(pub Request);

impl Message<CallRule> for HotUpgradeActor {
  type Reply = Result<Response>;

  async fn handle(
    &mut self,
    msg: CallRule,
    _ctx: &mut kameo::message::Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.rule.handle(&mut self.state, msg.0)
  }
}

pub struct UpgradeRule {
  pub wasm_path: PathBuf,
}

impl Message<UpgradeRule> for HotUpgradeActor {
  type Reply = Result<String>;

  async fn handle(
    &mut self,
    msg: UpgradeRule,
    _ctx: &mut kameo::message::Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.upgrade(msg.wasm_path)
  }
}

pub struct InspectRule {
  pub sample: Request,
}

impl Message<InspectRule> for HotUpgradeActor {
  type Reply = Result<RuleInspection>;

  async fn handle(
    &mut self,
    msg: InspectRule,
    _ctx: &mut kameo::message::Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.rule.inspect(msg.sample)
  }
}

pub struct Snapshot;

impl Message<Snapshot> for HotUpgradeActor {
  type Reply = Result<ServiceSnapshot>;

  async fn handle(
    &mut self,
    _msg: Snapshot,
    _ctx: &mut kameo::message::Context<Self, Self::Reply>,
  ) -> Self::Reply {
    Ok(self.snapshot())
  }
}

pub struct StartedHotUpgradeActor {
  actor_ref: ActorRef<HotUpgradeActor>,
}

impl StartedHotUpgradeActor {
  pub fn actor_ref(&self) -> ActorRef<HotUpgradeActor> {
    self.actor_ref.clone()
  }

  pub async fn shutdown(self) -> Result<()> {
    self
      .actor_ref
      .stop_gracefully()
      .await
      .map_err(|err| anyhow!("failed to stop hot-upgrade actor: {err}"))?;
    self.actor_ref.wait_for_shutdown().await;
    Ok(())
  }
}

pub fn start_hot_upgrade_actor(initial_rule: impl AsRef<Path>) -> Result<StartedHotUpgradeActor> {
  let actor = HotUpgradeActor::load(initial_rule)?;
  let actor_ref = HotUpgradeActor::spawn(actor);
  Ok(StartedHotUpgradeActor { actor_ref })
}

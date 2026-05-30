use std::path::{Path, PathBuf};

use anyhow::{Context as AnyhowContext, Result, anyhow};
use kameo::prelude::*;
use wasmtime::{Config, Engine};

use crate::{
  types::{
    Decision, Request, Response, RuleInspection, ServiceSnapshot, ServiceSnapshotV1,
    ServiceSnapshotV2, State, StateV2Stats,
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
    migrate_state(rule.required_schema(), &mut state)?;

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
    migrate_state(next.required_schema(), &mut self.state)?;
    self.state.upgrades += 1;
    self.rule = next;

    Ok(format!("upgrade {old_version} -> {}", self.rule.version()))
  }

  fn validate(&self, next: &mut WasmRule) -> Result<()> {
    let mut shadow = self.state.clone();
    migrate_state(next.required_schema(), &mut shadow)?;

    let response = next.handle(Request {
      user_id: 2,
      amount: 100,
      merchant_risk: 5,
      hour: 12,
    })?;

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

fn migrate_state(required_schema: u32, state: &mut State) -> Result<()> {
  while state.schema_version < required_schema {
    match state.schema_version {
      0 => {
        state.schema_version = 1;
      }
      1 => {
        state.fast_lane_hits = 0;
        state.v2 = Some(StateV2Stats {
          migration_generation: state.upgrades + 1,
          legacy_processed_at_migration: state.processed,
          ..Default::default()
        });
        state.schema_version = 2;
      }
      current => {
        return Err(anyhow!(
          "missing migrator for state schema {current} -> {}",
          current + 1
        ));
      }
    }
  }

  if state.schema_version > required_schema {
    return Err(anyhow!(
      "rule requires schema {required_schema}, but state is already at schema {}",
      state.schema_version,
    ));
  }

  Ok(())
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
    if self.state.schema_version != self.rule.required_schema() {
      return Err(anyhow!(
        "rule {} expects state schema {}, got {}",
        self.rule.version(),
        self.rule.required_schema(),
        self.state.schema_version,
      ));
    }

    let request = msg.0;
    let response = self.rule.handle(request.clone())?;

    self.state.processed += 1;
    self.state.last_score = response.risk_score;
    self.state.total_score += i64::from(response.risk_score);

    match &response.decision {
      Decision::Allow => self.state.allow_count += 1,
      Decision::AllowFastLane => {
        self.state.allow_count += 1;
        self.state.fast_lane_hits += 1;
      }
      Decision::Review => self.state.review_count += 1,
    }

    if let Some(v2) = &mut self.state.v2 {
      v2.last_decision = response.decision.clone();
      v2.last_policy_id = response.policy_id;
      v2.largest_amount = v2.largest_amount.max(request.amount);

      if request.merchant_risk >= 80 {
        v2.high_risk_requests += 1;
      }

      match &response.decision {
        Decision::AllowFastLane => v2.fast_lane_amount += request.amount,
        Decision::Review => {
          v2.reviewed_amount += request.amount;
          if request.hour <= 5 {
            v2.late_night_reviews += 1;
          }
        }
        Decision::Allow => {}
      }
    }

    Ok(response)
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

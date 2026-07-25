use std::path::{Path, PathBuf};

use anyhow::{Context as AnyhowContext, Result, anyhow};
use kameo::prelude::*;
use wasmtime::{Config, Engine};

use crate::{
  state::{ServiceSnapshot, ServiceState},
  types::{Request, Response, RuleInspection},
  wasm_rule::WasmRuleMethods,
};

#[derive(Actor)]
pub struct HotUpgradeActor {
  engine: Engine,
  state: ServiceState,
  rule_methods: WasmRuleMethods,
}

impl HotUpgradeActor {
  fn load(initial_rule: impl AsRef<Path>) -> Result<Self> {
    let initial_rule = initial_rule.as_ref();
    let engine = component_engine()?;
    let rule_methods = WasmRuleMethods::load(&engine, initial_rule).with_context(|| {
      format!(
        "failed to load initial wasm rule {}",
        initial_rule.display()
      )
    })?;

    let mut state = ServiceState::default();
    state.migrate_to_schema(rule_methods.required_schema())?;

    Ok(Self {
      engine,
      state,
      rule_methods,
    })
  }

  fn upgrade(&mut self, wasm_path: PathBuf) -> Result<String> {
    let old_version = self.rule_methods.version().to_owned();
    let mut next = WasmRuleMethods::load(&self.engine, &wasm_path)?;

    self.validate(&mut next)?;
    self.state.migrate_to_schema(next.required_schema())?;
    self.state.record_upgrade();
    self.rule_methods = next;

    Ok(format!(
      "upgrade {old_version} -> {}",
      self.rule_methods.version()
    ))
  }

  fn validate(&self, next: &mut WasmRuleMethods) -> Result<()> {
    let mut shadow = self.state.clone();
    shadow.migrate_to_schema(next.required_schema())?;

    let response = next.handle(
      Request {
        user_id: 2,
        amount: 100,
        merchant_risk: 5,
        hour: 12,
      },
      &mut shadow,
    )?;

    if response.rule_version.trim().is_empty() {
      return Err(anyhow!("new wasm rule returned an empty version"));
    }

    Ok(())
  }

  fn snapshot(&self) -> ServiceSnapshot {
    self.state.snapshot(self.rule_methods.version())
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
    self.state.ensure_schema(
      self.rule_methods.version(),
      self.rule_methods.required_schema(),
    )?;

    let request = msg.0;
    let response = self.rule_methods.handle(request, &mut self.state)?;

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
    self.rule_methods.inspect(msg.sample, &self.state)
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

use std::path::Path;

use anyhow::{Result, anyhow};
use wasmtime::{
  Engine, Store,
  component::{Component, Linker},
};

use crate::{
  bindings,
  types::{Decision, Request, Response, RuleInspection, RuleMetadata, State},
};

pub struct WasmRule {
  version: String,
  required_schema: u32,
  store: Store<()>,
  instance: bindings::RiskRule,
}

impl WasmRule {
  pub fn load(engine: &Engine, path: impl AsRef<Path>) -> Result<Self> {
    let path = path.as_ref();
    let component = Component::from_file(engine, path)
      .map_err(|err| anyhow!("failed to load wasm component {}: {err}", path.display()))?;
    let mut store = Store::new(engine, ());
    let linker = Linker::new(engine);
    let instance =
      bindings::RiskRule::instantiate(&mut store, &component, &linker).map_err(|err| {
        anyhow!(
          "failed to instantiate wasm component {}: {err}",
          path.display()
        )
      })?;

    let metadata = instance.rule().call_metadata(&mut store)?;
    if metadata.required_schema < 1 {
      return Err(anyhow!(
        "wasm module {} returned invalid schema {required_schema}",
        path.display(),
        required_schema = metadata.required_schema,
      ));
    }

    let version = path
      .file_stem()
      .and_then(|name| name.to_str())
      .unwrap_or("unknown")
      .to_owned();

    Ok(Self {
      version,
      required_schema: metadata.required_schema,
      store,
      instance,
    })
  }

  pub fn version(&self) -> &str {
    &self.version
  }

  pub fn required_schema(&self) -> u32 {
    self.required_schema
  }

  pub fn metadata(&mut self) -> Result<RuleMetadata> {
    let metadata = self.instance.rule().call_metadata(&mut self.store)?;
    Ok(RuleMetadata {
      version: self.version.clone(),
      required_schema: metadata.required_schema,
      policy_id: metadata.policy_id,
      dependency_marker: metadata.dependency_marker,
      review_threshold: metadata.review_threshold,
      fast_lane_limit: metadata.fast_lane_limit,
    })
  }

  pub fn inspect(&mut self, request: Request) -> Result<RuleInspection> {
    let sample_score = self.score(&request)?;
    Ok(RuleInspection {
      metadata: self.metadata()?,
      sample_request: request,
      sample_score,
    })
  }

  fn score(&mut self, request: &Request) -> Result<i32> {
    self
      .instance
      .rule()
      .call_risk_score(&mut self.store, request.into())
      .map_err(Into::into)
  }

  pub fn handle(&mut self, state: &mut State, request: Request) -> Result<Response> {
    if state.schema_version != self.required_schema {
      return Err(anyhow!(
        "rule {} expects state schema {}, got {}",
        self.version,
        self.required_schema,
        state.schema_version,
      ));
    }

    state.processed += 1;

    let risk_score = self.score(&request)?;
    let decision = match self
      .instance
      .rule()
      .call_decide(&mut self.store, (&request).into())?
    {
      bindings::exports::rule::Decision::Allow => Decision::Allow,
      bindings::exports::rule::Decision::Review => Decision::Review,
      bindings::exports::rule::Decision::AllowFastLane => {
        state.fast_lane_hits += 1;
        Decision::AllowFastLane
      }
    };

    match decision {
      Decision::Allow | Decision::AllowFastLane => state.allow_count += 1,
      Decision::Review => state.review_count += 1,
    }
    state.last_score = risk_score;
    state.total_score += i64::from(risk_score);

    let policy_id = self
      .instance
      .rule()
      .call_metadata(&mut self.store)?
      .policy_id;
    if let Some(v2) = &mut state.v2 {
      v2.last_decision = decision.clone();
      v2.last_policy_id = policy_id;
      v2.largest_amount = v2.largest_amount.max(request.amount);

      if request.merchant_risk >= 80 {
        v2.high_risk_requests += 1;
      }

      match &decision {
        Decision::AllowFastLane => {
          v2.fast_lane_amount += request.amount;
        }
        Decision::Review => {
          v2.reviewed_amount += request.amount;
          if request.hour <= 5 {
            v2.late_night_reviews += 1;
          }
        }
        Decision::Allow => {}
      }
    }

    Ok(Response {
      decision,
      rule_version: self.version.clone(),
      policy_id,
      risk_score,
    })
  }

  pub fn migrate_state(&self, state: &mut State) -> Result<()> {
    while state.schema_version < self.required_schema {
      match state.schema_version {
        0 => {
          state.schema_version = 1;
        }
        1 => {
          state.fast_lane_hits = 0;
          state.v2 = Some(crate::types::StateV2Stats {
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

    if state.schema_version > self.required_schema {
      return Err(anyhow!(
        "rule {} requires schema {}, but state is already at schema {}",
        self.version,
        self.required_schema,
        state.schema_version,
      ));
    }

    Ok(())
  }
}

impl From<&Request> for bindings::exports::rule::Request {
  fn from(request: &Request) -> Self {
    Self {
      user_id: request.user_id,
      amount: request.amount,
      merchant_risk: request.merchant_risk,
      hour: request.hour,
    }
  }
}

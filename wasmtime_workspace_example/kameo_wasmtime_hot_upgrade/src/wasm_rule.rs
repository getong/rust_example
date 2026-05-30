use std::path::Path;

use anyhow::{Result, anyhow};
use wasmtime::{Engine, Instance, Module, Store, TypedFunc};

use crate::types::{Decision, Request, Response, RuleInspection, RuleMetadata, State};

pub struct WasmRule {
  version: String,
  required_schema: u32,
  store: Store<()>,
  decide: TypedFunc<(i64, i64, i32, i32), i32>,
  risk_score: TypedFunc<(i64, i64, i32, i32), i32>,
  review_threshold: TypedFunc<(), i32>,
  fast_lane_limit: TypedFunc<(), i64>,
  policy_id: TypedFunc<(), i32>,
  dependency_marker: TypedFunc<(), i32>,
}

impl WasmRule {
  pub fn load(engine: &Engine, path: impl AsRef<Path>) -> Result<Self> {
    let path = path.as_ref();
    let module = Module::from_file(engine, path)
      .map_err(|err| anyhow!("failed to load wasm module {}: {err}", path.display()))?;
    let mut store = Store::new(engine, ());
    let instance = Instance::new(&mut store, &module, &[]).map_err(|err| {
      anyhow!(
        "failed to instantiate wasm module {}: {err}",
        path.display()
      )
    })?;

    let decide = instance
      .get_typed_func::<(i64, i64, i32, i32), i32>(&mut store, "decide")
      .map_err(|err| {
        anyhow!("wasm module must export `decide(i64, i64, i32, i32) -> i32`: {err}")
      })?;

    let risk_score = instance
      .get_typed_func::<(i64, i64, i32, i32), i32>(&mut store, "risk_score")
      .map_err(|err| {
        anyhow!("wasm module must export `risk_score(i64, i64, i32, i32) -> i32`: {err}")
      })?;

    let required_schema_func = instance
      .get_typed_func::<(), i32>(&mut store, "required_schema")
      .map_err(|err| anyhow!("wasm module must export `required_schema() -> i32`: {err}"))?;
    let required_schema = required_schema_func.call(&mut store, ())?;
    if required_schema < 1 {
      return Err(anyhow!(
        "wasm module {} returned invalid schema {required_schema}",
        path.display()
      ));
    }

    let review_threshold = instance
      .get_typed_func::<(), i32>(&mut store, "review_threshold")
      .map_err(|err| anyhow!("wasm module must export `review_threshold() -> i32`: {err}"))?;
    let fast_lane_limit = instance
      .get_typed_func::<(), i64>(&mut store, "fast_lane_limit")
      .map_err(|err| anyhow!("wasm module must export `fast_lane_limit() -> i64`: {err}"))?;
    let policy_id = instance
      .get_typed_func::<(), i32>(&mut store, "policy_id")
      .map_err(|err| anyhow!("wasm module must export `policy_id() -> i32`: {err}"))?;
    let dependency_marker = instance
      .get_typed_func::<(), i32>(&mut store, "dependency_marker")
      .map_err(|err| anyhow!("wasm module must export `dependency_marker() -> i32`: {err}"))?;

    let version = path
      .file_stem()
      .and_then(|name| name.to_str())
      .unwrap_or("unknown")
      .to_owned();

    Ok(Self {
      version,
      required_schema: required_schema as u32,
      store,
      decide,
      risk_score,
      review_threshold,
      fast_lane_limit,
      policy_id,
      dependency_marker,
    })
  }

  pub fn version(&self) -> &str {
    &self.version
  }

  pub fn required_schema(&self) -> u32 {
    self.required_schema
  }

  pub fn metadata(&mut self) -> Result<RuleMetadata> {
    Ok(RuleMetadata {
      version: self.version.clone(),
      required_schema: self.required_schema,
      policy_id: self.policy_id.call(&mut self.store, ())?,
      dependency_marker: self.dependency_marker.call(&mut self.store, ())?,
      review_threshold: self.review_threshold.call(&mut self.store, ())?,
      fast_lane_limit: self.fast_lane_limit.call(&mut self.store, ())?,
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
      .risk_score
      .call(
        &mut self.store,
        (
          request.user_id,
          request.amount,
          request.merchant_risk,
          request.hour,
        ),
      )
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
    let code = self.decide.call(
      &mut self.store,
      (
        request.user_id,
        request.amount,
        request.merchant_risk,
        request.hour,
      ),
    )?;
    let decision = match code {
      0 => Decision::Allow,
      1 => Decision::Review,
      2 => {
        state.fast_lane_hits += 1;
        Decision::AllowFastLane
      }
      other => return Err(anyhow!("unknown decision code from wasm: {other}")),
    };

    match decision {
      Decision::Allow | Decision::AllowFastLane => state.allow_count += 1,
      Decision::Review => state.review_count += 1,
    }
    state.last_score = risk_score;
    state.total_score += i64::from(risk_score);

    let policy_id = self.policy_id.call(&mut self.store, ())?;
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

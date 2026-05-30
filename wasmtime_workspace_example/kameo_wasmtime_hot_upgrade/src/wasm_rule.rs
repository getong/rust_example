use std::path::Path;

use anyhow::{Result, anyhow};
use wasmtime::{
  Engine, Store,
  component::{Component, Linker},
};

use crate::{
  bindings,
  types::{Decision, Request, Response, RuleInspection, RuleMetadata},
};

pub struct WasmRuleMethods {
  version: String,
  required_schema: u32,
  store: Store<()>,
  instance: bindings::RiskRule,
}

impl WasmRuleMethods {
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
    let sample_score = self.evaluate(&request)?.risk_score;
    Ok(RuleInspection {
      metadata: self.metadata()?,
      sample_request: request,
      sample_score,
    })
  }

  fn evaluate(&mut self, request: &Request) -> Result<bindings::exports::rule::Evaluation> {
    self
      .instance
      .rule()
      .call_evaluate(&mut self.store, request.into())
      .map_err(Into::into)
  }

  pub fn handle(&mut self, request: Request) -> Result<Response> {
    let evaluation = self.evaluate(&request)?;
    Ok(Response {
      decision: evaluation.decision.into(),
      rule_version: self.version.clone(),
      policy_id: evaluation.policy_id,
      risk_score: evaluation.risk_score,
    })
  }
}

pub type WasmRule = WasmRuleMethods;

impl From<bindings::exports::rule::Decision> for Decision {
  fn from(decision: bindings::exports::rule::Decision) -> Self {
    match decision {
      bindings::exports::rule::Decision::Allow => Self::Allow,
      bindings::exports::rule::Decision::Review => Self::Review,
      bindings::exports::rule::Decision::AllowFastLane => Self::AllowFastLane,
    }
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

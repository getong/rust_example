use std::path::Path;

use anyhow::{Result, anyhow};
use wasmtime::{
  Engine, Store,
  component::{Component, Linker},
};

use crate::{
  bindings,
  types::{Decision, Request, Response, RuleInspection, RuleMetadata, RuleRuntimeSnapshot},
};

#[derive(Debug, Clone)]
struct WasmRuleStoreState {
  version: String,
  component_path: String,
  loaded_metadata: RuleMetadata,
  metadata_calls: u64,
  evaluate_calls: u64,
  last_request: Option<Request>,
  last_response: Option<Response>,
}

pub struct WasmRuleMethods {
  version: String,
  required_schema: u32,
  store: Store<WasmRuleStoreState>,
  instance: bindings::RiskRule,
}

impl WasmRuleMethods {
  pub fn load(engine: &Engine, path: impl AsRef<Path>) -> Result<Self> {
    let path = path.as_ref();
    let component = Component::from_file(engine, path)
      .map_err(|err| anyhow!("failed to load wasm component {}: {err}", path.display()))?;
    let version = path
      .file_stem()
      .and_then(|name| name.to_str())
      .unwrap_or("unknown")
      .to_owned();

    let placeholder_metadata = RuleMetadata {
      version: version.clone(),
      required_schema: 0,
      policy_id: 0,
      dependency_marker: 0,
      review_threshold: 0,
      fast_lane_limit: 0,
    };
    let mut store = Store::new(
      engine,
      WasmRuleStoreState {
        version: version.clone(),
        component_path: path.display().to_string(),
        loaded_metadata: placeholder_metadata,
        metadata_calls: 0,
        evaluate_calls: 0,
        last_request: None,
        last_response: None,
      },
    );
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

    store.data_mut().metadata_calls += 1;
    store.data_mut().loaded_metadata = RuleMetadata {
      version: version.clone(),
      required_schema: metadata.required_schema,
      policy_id: metadata.policy_id,
      dependency_marker: metadata.dependency_marker,
      review_threshold: metadata.review_threshold,
      fast_lane_limit: metadata.fast_lane_limit,
    };

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
    self.store.data_mut().metadata_calls += 1;
    let metadata = RuleMetadata {
      version: self.version.clone(),
      required_schema: metadata.required_schema,
      policy_id: metadata.policy_id,
      dependency_marker: metadata.dependency_marker,
      review_threshold: metadata.review_threshold,
      fast_lane_limit: metadata.fast_lane_limit,
    };
    self.store.data_mut().loaded_metadata = metadata.clone();
    Ok(metadata)
  }

  pub fn inspect(&mut self, request: Request) -> Result<RuleInspection> {
    let metadata = self.metadata()?;
    let sample_response = self.handle(request.clone())?;
    let runtime = self.runtime_snapshot();
    Ok(RuleInspection {
      metadata,
      sample_request: request,
      sample_score: sample_response.risk_score,
      runtime,
    })
  }

  fn evaluate(&mut self, request: &Request) -> Result<bindings::exports::rule::Evaluation> {
    let evaluation = self
      .instance
      .rule()
      .call_evaluate(&mut self.store, request.into())?;
    self.store.data_mut().evaluate_calls += 1;
    self.store.data_mut().last_request = Some(request.clone());
    Ok(evaluation)
  }

  pub fn handle(&mut self, request: Request) -> Result<Response> {
    let evaluation = self.evaluate(&request)?;
    let response = Response {
      decision: evaluation.decision.into(),
      rule_version: self.version.clone(),
      policy_id: evaluation.policy_id,
      risk_score: evaluation.risk_score,
    };
    self.store.data_mut().last_response = Some(response.clone());
    Ok(response)
  }

  pub fn runtime_snapshot(&self) -> RuleRuntimeSnapshot {
    let state = self.store.data();
    RuleRuntimeSnapshot {
      version: state.version.clone(),
      component_path: state.component_path.clone(),
      loaded_required_schema: state.loaded_metadata.required_schema,
      metadata_calls: state.metadata_calls,
      evaluate_calls: state.evaluate_calls,
      last_request: state.last_request.clone(),
      last_response: state.last_response.clone(),
    }
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

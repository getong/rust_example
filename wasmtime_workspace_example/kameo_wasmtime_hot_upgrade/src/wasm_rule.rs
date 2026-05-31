use std::{fs, path::Path};

use anyhow::{Result, anyhow};
use wasmtime::{
  Engine, Store,
  component::{Component, HasSelf, Linker},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::{
  bindings::{
    self,
    host::{self, Host},
  },
  state::ServiceState,
  types::{Decision, Request, Response, RuleInspection, RuleMetadata, RuleRuntimeSnapshot},
};

pub struct WasmRuleStoreState {
  table: ResourceTable,
  wasi: WasiCtx,
  pub version: String,
  pub component_path: String,
  pub loaded_metadata: RuleMetadata,
  pub host_method_entries: u64,
  pub last_host_method: Option<String>,
  pub last_host_policy_id: Option<i32>,
  pub metadata_calls: u64,
  pub evaluate_calls: u64,
  pub last_request: Option<Request>,
  pub last_response: Option<Response>,
  /// Score pushed by the wasm module via `host::record_last_score`.
  pub last_score: Option<i32>,
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
    let wasm_bytes = fs::read(path)
      .map_err(|err| anyhow!("failed to read wasm component {}: {err}", path.display()))?;
    validate_wasm_component(path, &wasm_bytes)?;
    let component = Component::new(engine, &wasm_bytes)
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
        table: ResourceTable::new(),
        wasi: WasiCtxBuilder::new().build(),
        version: version.clone(),
        component_path: path.display().to_string(),
        loaded_metadata: placeholder_metadata,
        host_method_entries: 0,
        last_host_method: None,
        last_host_policy_id: None,
        metadata_calls: 0,
        evaluate_calls: 0,
        last_request: None,
        last_response: None,
        last_score: None,
      },
    );
    let mut linker = Linker::new(engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
    host::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;
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

  /// Read-only access to the runtime state inside the wasmtime `Store`.
  pub fn store_state(&self) -> &WasmRuleStoreState {
    self.store.data()
  }

  /// Mutable access to the runtime state inside the wasmtime `Store`.
  pub fn store_state_mut(&mut self) -> &mut WasmRuleStoreState {
    self.store.data_mut()
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

  pub fn inspect(&mut self, request: Request, state: &ServiceState) -> Result<RuleInspection> {
    let metadata = self.metadata()?;
    let mut shadow = state.clone();
    let sample_response = self.handle(request.clone(), &mut shadow)?;
    let runtime = self.runtime_snapshot();
    Ok(RuleInspection {
      metadata,
      sample_request: request,
      sample_score: sample_response.risk_score,
      runtime,
    })
  }

  fn evaluate(
    &mut self,
    request: &Request,
    state: &ServiceState,
  ) -> Result<bindings::exports::rule::EvaluateResult> {
    let result = self.instance.rule().call_evaluate(
      &mut self.store,
      request.into(),
      state.to_component_state(),
    )?;
    self.store.data_mut().evaluate_calls += 1;
    self.store.data_mut().last_request = Some(request.clone());
    Ok(result)
  }

  pub fn handle(&mut self, request: Request, state: &mut ServiceState) -> Result<Response> {
    let result = self.evaluate(&request, state)?;
    let mut updated_state = state.clone();
    updated_state.save_component_state(result.state);
    updated_state.ensure_schema(&self.version, self.required_schema)?;
    *state = updated_state;

    let evaluation = result.evaluation;
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
      host_method_entries: state.host_method_entries,
      last_host_method: state.last_host_method.clone(),
      last_host_policy_id: state.last_host_policy_id,
      metadata_calls: state.metadata_calls,
      evaluate_calls: state.evaluate_calls,
      last_request: state.last_request.clone(),
      last_response: state.last_response.clone(),
      last_score: state.last_score,
    }
  }
}

fn validate_wasm_component(path: &Path, wasm_bytes: &[u8]) -> Result<()> {
  wasmparser::Validator::new()
    .validate_all(wasm_bytes)
    .map(|_| ())
    .map_err(|err| anyhow!("invalid wasm component {}: {err}", path.display()))
}

impl WasiView for WasmRuleStoreState {
  fn ctx(&mut self) -> WasiCtxView<'_> {
    WasiCtxView {
      ctx: &mut self.wasi,
      table: &mut self.table,
    }
  }
}

impl Host for WasmRuleStoreState {
  fn method_enter(&mut self, method: String, policy_id: i32) {
    self.host_method_entries += 1;
    self.last_host_method = Some(method);
    self.last_host_policy_id = Some(policy_id);
  }

  fn loaded_required_schema(&mut self) -> u32 {
    self.loaded_metadata.required_schema
  }

  fn evaluate_call_count(&mut self) -> u64 {
    self.evaluate_calls
  }

  fn record_last_score(&mut self, score: i32) {
    self.last_score = Some(score);
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

use std::path::Path;

use anyhow::{Result, anyhow};
use wasmtime::{Engine, Instance, Module, Store, TypedFunc};

use crate::types::{Decision, Request, Response, State};

/// Wraps a loaded `.wasm` rule module and exposes `handle` / `migrate_state`.
///
/// The WASM module must export two functions:
///
/// ```text
/// required_schema() -> i32
/// decide(user_id: i64, amount: i64) -> i32
/// ```
///
/// Decision codes:
/// * `0` – allow
/// * `1` – review
/// * `2` – allow-fast-lane
pub struct WasmHandler {
  version: String,
  required_schema: u32,
  store: Store<()>,
  decide: TypedFunc<(i64, i64), i32>,
}

impl WasmHandler {
  /// Load a `.wasm` file and resolve its exported functions.
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
      .get_typed_func::<(i64, i64), i32>(&mut store, "decide")
      .map_err(|err| anyhow!("wasm module must export `decide(i64, i64) -> i32`: {err}"))?;

    let required_schema_func = instance
      .get_typed_func::<(), i32>(&mut store, "required_schema")
      .map_err(|err| anyhow!("wasm module must export `required_schema() -> i32`: {err}"))?;
    let required_schema = required_schema_func.call(&mut store, ())? as u32;

    let version = path
      .file_stem()
      .and_then(|n| n.to_str())
      .unwrap_or("unknown")
      .to_string();

    Ok(Self {
      version,
      required_schema,
      store,
      decide,
    })
  }

  pub fn version(&self) -> &str {
    &self.version
  }

  /// Evaluate the rule against `request`, updating `state` in-place.
  ///
  /// Fails if `state.schema_version` does not match what the module requires.
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

    let code = self
      .decide
      .call(&mut self.store, (request.user_id, request.amount))?;

    let decision = match code {
      0 => Decision::Allow,
      1 => Decision::Review,
      2 => {
        state.fast_lane_hits += 1;
        Decision::AllowFastLane
      }
      other => return Err(anyhow!("unknown decision code from wasm: {other}")),
    };

    Ok(Response {
      decision,
      rule_version: self.version.clone(),
    })
  }

  /// Migrate `state` forward until `state.schema_version == required_schema`.
  pub fn migrate_state(&self, state: &mut State) -> Result<()> {
    while state.schema_version < self.required_schema {
      match state.schema_version {
        0 => {
          state.schema_version = 1;
        }
        1 => {
          // Schema 2 starts tracking fast-lane hits explicitly.
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

use std::path::Path;

use anyhow::{anyhow, Result};
use wasmer::{imports, Instance, Module, Store, TypedFunction};

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
  store: Store,
  decide: TypedFunction<(i64, i64), i32>,
}

impl WasmHandler {
  /// Load a `.wasm` file and resolve its exported functions.
  pub fn load(path: impl AsRef<Path>) -> Result<Self> {
    let path = path.as_ref();
    let mut store = Store::default();

    let wasm_bytes = std::fs::read(path)
      .map_err(|err| anyhow!("failed to read wasm module {}: {err}", path.display()))?;
    let module = Module::new(&store, wasm_bytes)
      .map_err(|err| anyhow!("failed to compile wasm module {}: {err}", path.display()))?;

    let import_object = imports! {};
    let instance = Instance::new(&mut store, &module, &import_object).map_err(|err| {
      anyhow!(
        "failed to instantiate wasm module {}: {err}",
        path.display()
      )
    })?;

    let decide: TypedFunction<(i64, i64), i32> = instance
      .exports
      .get_typed_function(&store, "decide")
      .map_err(|err| anyhow!("wasm module must export `decide(i64, i64) -> i32`: {err}"))?;

    let required_schema_func: TypedFunction<(), i32> = instance
      .exports
      .get_typed_function(&store, "required_schema")
      .map_err(|err| anyhow!("wasm module must export `required_schema() -> i32`: {err}"))?;
    let required_schema = required_schema_func.call(&mut store)? as u32;

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
      .call(&mut self.store, request.user_id, request.amount)?;

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

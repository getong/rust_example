//! Demonstration of running Rust-compiled WASM rules with the Wasmer runtime.
//!
//! Modeled after the `hot_upgrade` wasmtime example: two rule crates
//! (`risk_rule_v1`, `risk_rule_v2`) are compiled to `wasm32-unknown-unknown`
//! and the host loads them with Wasmer, hot-swapping from v1 to v2 while
//! migrating the in-memory state.
//!
//! The guest modules are built automatically on `cargo run`; to build them
//! manually:
//!
//! ```sh
//! rustup target add wasm32-unknown-unknown
//! cargo build --package risk_rule_v1 --release --target wasm32-unknown-unknown
//! cargo build --package risk_rule_v2 --release --target wasm32-unknown-unknown
//! ```

mod types;
mod wasm_rule;

use std::{
  path::{Path, PathBuf},
  process::Command,
};

use anyhow::{anyhow, Context, Result};
use types::{Request, State};
use wasm_rule::WasmHandler;

const GUEST_TARGET: &str = "wasm32-unknown-unknown";

fn manifest_dir() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Build a guest rule crate for `wasm32-unknown-unknown` and return the
/// path of the produced `.wasm` artifact.
fn ensure_guest_wasm(package: &str) -> Result<PathBuf> {
  let manifest_dir = manifest_dir();
  let wasm_path = manifest_dir
    .join("target")
    .join(GUEST_TARGET)
    .join("release")
    .join(format!("{package}.wasm"));

  let output = Command::new("cargo")
    .current_dir(&manifest_dir)
    .args([
      "build",
      "--package",
      package,
      "--release",
      "--target",
      GUEST_TARGET,
    ])
    .output()
    .context("failed to invoke cargo to build the guest wasm module")?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains(&format!("the `{GUEST_TARGET}` target may not be installed")) {
      return Err(anyhow!(
        "the `{GUEST_TARGET}` target is missing; run `rustup target add {GUEST_TARGET}` and rerun \
         `cargo run`.\n\ncargo stderr:\n{stderr}"
      ));
    }
    return Err(anyhow!(
      "failed to build guest package {package}:\n{stderr}"
    ));
  }

  if !wasm_path.is_file() {
    return Err(anyhow!(
      "cargo reported success, but the guest wasm module was not produced at {}",
      wasm_path.display()
    ));
  }

  Ok(wasm_path)
}

fn run_requests(handler: &mut WasmHandler, state: &mut State, requests: &[Request]) -> Result<()> {
  for request in requests {
    let response = handler.handle(state, request.clone())?;
    println!(
      "user_id={} amount={} -> {}:{}",
      request.user_id, request.amount, response.rule_version, response.decision
    );
  }
  Ok(())
}

/// Load a new rule module and migrate `state` to its required schema,
/// returning the new handler only if both steps succeed.
fn upgrade(state: &mut State, wasm_path: &Path) -> Result<WasmHandler> {
  let handler = WasmHandler::load(wasm_path)?;

  // Validate the migration against a shadow copy first, so a broken
  // migrator cannot corrupt the live state.
  let mut shadow = state.clone();
  handler.migrate_state(&mut shadow)?;

  *state = shadow;
  state.upgrades += 1;
  Ok(handler)
}

fn main() -> Result<()> {
  let v1_wasm = ensure_guest_wasm("risk_rule_v1")?;
  let v2_wasm = ensure_guest_wasm("risk_rule_v2")?;

  // -------------------------------------------------------------------------
  // T0: Service starts – only the v1 wasm rule is active.
  // -------------------------------------------------------------------------
  let mut state = State::default();
  let mut handler = WasmHandler::load(&v1_wasm)?;
  handler.migrate_state(&mut state)?;

  println!("--- {} rule active (wasmer) ---", handler.version());
  run_requests(
    &mut handler,
    &mut state,
    &[
      Request {
        user_id: 1,
        amount: 6_000,
      },
      Request {
        user_id: 9,
        amount: 12_000,
      },
    ],
  )?;

  // -------------------------------------------------------------------------
  // T1: Hot-swap to the v2 rule: load the new module, migrate the state
  // to schema 2, then replace the handler.
  // -------------------------------------------------------------------------
  handler = upgrade(&mut state, &v2_wasm)?;
  println!("\nupgraded to rule {}\n", handler.version());

  println!("--- {} rule active (wasmer) ---", handler.version());
  run_requests(
    &mut handler,
    &mut state,
    &[
      Request {
        user_id: 2,
        amount: 3_000,
      },
      Request {
        user_id: 3,
        amount: 6_000,
      },
    ],
  )?;

  println!(
    "\nprocessed={}, schema={}, fast_lane_hits={}, current_rule={}, upgrades={}",
    state.processed,
    state.schema_version,
    state.fast_lane_hits,
    handler.version(),
    state.upgrades,
  );

  Ok(())
}

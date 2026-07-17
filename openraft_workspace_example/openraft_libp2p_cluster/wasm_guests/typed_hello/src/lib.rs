//! Example typed task guest: exchanges typed WIT records with the worker
//! (no stdout scraping) and exercises the full `cluster:task/host`
//! surface — log, task-id, fuel-remaining, wall-clock-remaining-ms,
//! report-progress and config-get. Compare with `script/wat/*.wat`, the
//! classic WASI CLI guests — both module styles run on the same worker;
//! the host picks the path by detecting the `cluster:task/runner` export.
//!
//! Built as generation 1 (default) or generation 2 (`--features
//! guest-v2`), the wasmtime_actor hot-upgrade pattern: deploy the new
//! `.wasm` into the module store and the next task runs the new logic.

wit_bindgen::generate!({
  // The same WIT the host compiles via wasmtime::component::bindgen!.
  path: "../../wit/task-executor.wit",
  world: "task-executor",
});

use cluster::task::host;
use exports::cluster::task::runner::{Guest, TaskInput, TaskOutput};

#[cfg(feature = "guest-v2")]
const GENERATION: u32 = 2;
#[cfg(not(feature = "guest-v2"))]
const GENERATION: u32 = 1;

struct TypedHello;

impl Guest for TypedHello {
  fn run(input: TaskInput) -> Result<TaskOutput, String> {
    // Host diagnostics: the volatile values (fuel, wall clock) go to the
    // host log; the returned records below stay deterministic.
    let task_id = host::task_id();
    host::log(&format!(
      "typed_hello g{GENERATION} task={task_id:?}: {} args, {} env, fuel_left={}, wall_left_ms={}",
      input.args.len(),
      input.env.len(),
      host::fuel_remaining(),
      host::wall_clock_remaining_ms(),
    ));
    host::report_progress(0, 1);

    // Same shape as the digest/stats demos: parse argv, compute, return a
    // typed record — plus a host-config knob read via config-get.
    let n: u64 = input
      .args
      .first()
      .map(|raw| raw.parse().map_err(|err| format!("bad arg {raw:?}: {err}")))
      .transpose()?
      .unwrap_or(41);
    let bonus: u64 = host::config_get("bonus")
      .and_then(|raw| raw.parse().ok())
      .unwrap_or(0);
    let answer = n + 1 + bonus;
    host::report_progress(1, 1);

    // itoa keeps integer→string off format!'s heap machinery (§12).
    let mut n_buf = itoa::Buffer::new();
    let mut answer_buf = itoa::Buffer::new();
    let mut generation_buf = itoa::Buffer::new();
    let mut structured = String::with_capacity(64);
    structured.push_str(r#"{"input":"#);
    structured.push_str(n_buf.format(n));
    structured.push_str(r#","answer":"#);
    structured.push_str(answer_buf.format(answer));
    structured.push_str(r#","generation":"#);
    structured.push_str(generation_buf.format(GENERATION));
    structured.push('}');

    Ok(TaskOutput {
      stdout: format!("typed_hello: {n} + 1 = {answer}\n"),
      structured: Some(structured),
    })
  }
}

export!(TypedHello);

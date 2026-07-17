//! Example typed task guest: exchanges typed WIT records with the worker
//! (no stdout scraping) and calls back into the `cluster:task/host.log`
//! host function. Compare with `script/wat/*.wat`, the classic WASI CLI
//! guests — both module styles run on the same worker; the host picks the
//! path by detecting the `cluster:task/runner` export.

wit_bindgen::generate!({
  // The same WIT the host compiles via wasmtime::component::bindgen!.
  path: "../../wit/task-executor.wit",
  world: "task-executor",
});

use exports::cluster::task::runner::{Guest, TaskInput, TaskOutput};

struct TypedHello;

impl Guest for TypedHello {
  fn run(input: TaskInput) -> Result<TaskOutput, String> {
    cluster::task::host::log(&format!(
      "typed_hello: {} args, {} env vars",
      input.args.len(),
      input.env.len()
    ));

    // Same shape as the digest/stats demos: parse argv, compute, return a
    // typed record — the structured field carries machine-readable JSON
    // without the guest hand-assembling stdout protocols.
    let n: u64 = input
      .args
      .first()
      .map(|raw| raw.parse().map_err(|err| format!("bad arg {raw:?}: {err}")))
      .transpose()?
      .unwrap_or(41);
    let answer = n + 1;

    Ok(TaskOutput {
      stdout: format!("typed_hello: {n} + 1 = {answer}\n"),
      structured: Some(format!(r#"{{"input":{n},"answer":{answer}}}"#)),
    })
  }
}

export!(TypedHello);

use std::{
  path::{Path, PathBuf},
  process::Command,
  sync::Arc,
};

use anyhow::{Context, Result, bail};
use tokio::time::Duration;
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::{WasiCtx, p1::WasiP1Ctx};

const GUEST_TARGET: &str = "wasm32-wasip1";
const GUEST_BIN: &str = "tokio-wasi";

#[tokio::main]
async fn main() -> Result<()> {
  // Create an environment shared by all wasm execution. This contains
  // the `Engine` and the `Module` we are executing.
  let env = Environment::new()?;

  // The inputs to run_wasm are `Send`: we can create them here and send
  // them to a new task that we spawn.
  let inputs1 = Inputs::new(env.clone(), "Gussie");
  let inputs2 = Inputs::new(env.clone(), "Willa");
  let inputs3 = Inputs::new(env, "Sparky");

  // Spawn some tasks. Insert sleeps before run_wasm so that the
  // interleaving is easy to observe.
  let join1 = tokio::task::spawn(async move { run_wasm(inputs1).await });
  let join2 = tokio::task::spawn(async move {
    tokio::time::sleep(Duration::from_millis(750)).await;
    run_wasm(inputs2).await
  });
  let join3 = tokio::task::spawn(async move {
    tokio::time::sleep(Duration::from_millis(1250)).await;
    run_wasm(inputs3).await
  });

  // All tasks should join successfully.
  join1.await??;
  join2.await??;
  join3.await??;
  Ok(())
}

#[derive(Clone)]
struct Environment {
  engine: Engine,
  module: Module,
  linker: Arc<Linker<WasiP1Ctx>>,
}

impl Environment {
  pub fn new() -> Result<Self> {
    let mut config = Config::new();
    // Consume fuel for guests so that they can co-operatively yield during
    // execution.
    config.consume_fuel(true);

    let engine = Engine::new(&config)?;
    let module = Module::from_file(&engine, ensure_guest_wasm()?)?;

    // A `Linker` is shared in the environment amongst all stores, and this
    // linker is used to instantiate the `module` above. This example only
    // adds WASI functions to the linker, notably the async versions built
    // on tokio.
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::p1::add_to_linker_async(&mut linker, |cx| cx)?;

    Ok(Self {
      engine,
      module,
      linker: Arc::new(linker),
    })
  }
}

struct Inputs {
  env: Environment,
  name: String,
}

impl Inputs {
  fn new(env: Environment, name: &str) -> Self {
    Self {
      env,
      name: name.to_owned(),
    }
  }
}

async fn run_wasm(inputs: Inputs) -> Result<()> {
  let wasi = WasiCtx::builder()
    // Let wasi print to this process's stdout.
    .inherit_stdout()
    // Set an environment variable so the wasm knows its name.
    .env("NAME", &inputs.name)
    .build_p1();
  let mut store = Store::new(&inputs.env.engine, wasi);

  // Put effectively unlimited fuel so it can run forever.
  store.set_fuel(u64::MAX)?;
  // WebAssembly execution will be paused for an async yield every time it
  // consumes 10000 fuel.
  store.fuel_async_yield_interval(Some(10000))?;

  // Instantiate into our own unique store using the shared linker, afterwards
  // acquiring the `_start` function for the module and executing it.
  let instance = inputs
    .env
    .linker
    .instantiate_async(&mut store, &inputs.env.module)
    .await?;
  instance
    .get_typed_func::<(), ()>(&mut store, "_start")?
    .call_async(&mut store, ())
    .await?;

  Ok(())
}

fn ensure_guest_wasm() -> Result<PathBuf> {
  let package_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let workspace_dir = package_dir
    .parent()
    .context("failed to determine workspace root for wasmtime_tokio")?;
  let guest_wasm = workspace_dir
    .join("target")
    .join(GUEST_TARGET)
    .join("debug")
    .join(format!("{GUEST_BIN}.wasm"));

  let output = Command::new("cargo")
    .current_dir(package_dir)
    .arg("build")
    .arg("--target")
    .arg(GUEST_TARGET)
    .arg("--target-dir")
    .arg(workspace_dir.join("target"))
    .arg("--bin")
    .arg(GUEST_BIN)
    .output()
    .context("failed to invoke cargo to build the guest wasm module")?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stderr.contains("the `wasm32-wasip1` target may not be installed") {
      bail!(
        "failed to build `{GUEST_BIN}.wasm` for `{GUEST_TARGET}`.\ninstall the target first with \
         `rustup target add {GUEST_TARGET}` and rerun `cargo run --bin wasmtime_tokio`.\n\ncargo \
         stderr:\n{stderr}"
      );
    }

    bail!(
      "failed to build `{GUEST_BIN}.wasm` before launching the host example.\npackage dir: \
       {}\nexpected output: {}\n\ncargo stdout:\n{stdout}\n\ncargo stderr:\n{stderr}",
      package_dir.display(),
      guest_wasm.display(),
    );
  }

  if !guest_wasm.is_file() {
    bail!(
      "cargo reported success, but the guest wasm module was not produced at {}",
      guest_wasm.display(),
    );
  }

  Ok(guest_wasm)
}

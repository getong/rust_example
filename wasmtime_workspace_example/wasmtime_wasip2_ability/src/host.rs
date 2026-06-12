use std::{
  path::{Path, PathBuf},
  process::Command as ProcessCommand,
};

use anyhow::{Result, bail};
use wasmtime::{
  Config, Engine, Store,
  component::{Component, Linker, ResourceTable},
};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView, p2::bindings::Command};
use wasmtime_wasi_http::{
  WasiHttpCtx,
  p2::{WasiHttpCtxView, WasiHttpView},
};

const GUEST_TARGET: &str = "wasm32-wasip2";
const GUEST_WASM: &str = "wasmtime_wasip2_ability.wasm";

struct HostState {
  table: ResourceTable,
  wasi: WasiCtx,
  http: WasiHttpCtx,
}

impl WasiView for HostState {
  fn ctx(&mut self) -> WasiCtxView<'_> {
    WasiCtxView {
      ctx: &mut self.wasi,
      table: &mut self.table,
    }
  }
}

impl WasiHttpView for HostState {
  fn http(&mut self) -> WasiHttpCtxView<'_> {
    WasiHttpCtxView {
      ctx: &mut self.http,
      table: &mut self.table,
      hooks: Default::default(),
    }
  }
}

pub async fn run() -> Result<()> {
  let component_path = if let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) {
    path
  } else {
    ensure_guest_component()?
  };

  println!(
    "host: running WASIp2 component with wasi-http + wasi-sockets enabled: {}",
    component_path.display()
  );
  run_component(&component_path).await
}

async fn run_component(component_path: &Path) -> Result<()> {
  if !component_path.is_file() {
    bail!(
      "component path does not exist or is not a file: {}",
      component_path.display()
    );
  }

  let engine = component_engine()?;
  let mut linker = Linker::new(&engine);

  // Includes WASIp2 CLI, filesystem, clocks, random, and wasi:sockets.
  wasmtime_wasi::p2::add_to_linker_async(&mut linker)
    .map_err(|err| anyhow::anyhow!("failed to add WASIp2 interfaces to linker: {err}"))?;

  // Adds wasi:http interfaces without re-adding the WASI interfaces above.
  wasmtime_wasi_http::p2::add_only_http_to_linker_async(&mut linker)
    .map_err(|err| anyhow::anyhow!("failed to add wasi-http interfaces to linker: {err}"))?;

  let component = Component::from_file(&engine, component_path).map_err(|err| {
    anyhow::anyhow!(
      "failed to load component from {}: {err}",
      component_path.display()
    )
  })?;

  let mut store = Store::new(
    &engine,
    HostState {
      table: ResourceTable::new(),
      wasi: wasi_ctx_for_component(component_path),
      http: WasiHttpCtx::new(),
    },
  );

  let command = Command::instantiate_async(&mut store, &component, &linker)
    .await
    .map_err(|err| {
      anyhow::anyhow!(
        "failed to instantiate WASIp2 command component {}: {err}",
        component_path.display()
      )
    })?;

  command
    .wasi_cli_run()
    .call_run(&mut store)
    .await
    .map_err(|err| anyhow::anyhow!("failed while calling wasi:cli/run.run: {err}"))?
    .map_err(|()| anyhow::anyhow!("guest returned a failing WASI exit status"))
}

fn component_engine() -> Result<Engine> {
  let mut config = Config::new();
  config.wasm_component_model(true);
  Engine::new(&config).map_err(|err| anyhow::anyhow!("failed to create Wasmtime engine: {err}"))
}

fn wasi_ctx_for_component(component_path: &Path) -> WasiCtx {
  let mut builder = WasiCtxBuilder::new();
  builder
    .inherit_stdio()
    .arg(component_path.display().to_string())
    .inherit_network()
    .allow_tcp(true)
    .allow_udp(true)
    .allow_ip_name_lookup(true);
  builder.build()
}

fn ensure_guest_component() -> Result<PathBuf> {
  let package_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  let workspace_dir = package_dir
    .parent()
    .ok_or_else(|| anyhow::anyhow!("failed to determine workspace root"))?;
  let target_dir = workspace_dir
    .join("target")
    .join("wasmtime-wasip2-ability-guest");

  let status = ProcessCommand::new("cargo")
    .args([
      "build",
      "--package",
      "wasmtime_wasip2_ability",
      "--target",
      GUEST_TARGET,
      "--target-dir",
    ])
    .arg(&target_dir)
    .status()
    .map_err(|err| anyhow::anyhow!("failed to invoke cargo to build guest component: {err}"))?;

  if !status.success() {
    bail!(
      "failed to build the embedded WASIp2 guest. Install the target with `rustup target add \
       {GUEST_TARGET}` and rerun `cargo run`"
    );
  }

  let component = target_dir.join(GUEST_TARGET).join("debug").join(GUEST_WASM);
  if !component.is_file() {
    bail!(
      "cargo reported success, but guest component was not produced at {}",
      component.display()
    );
  }

  Ok(component)
}

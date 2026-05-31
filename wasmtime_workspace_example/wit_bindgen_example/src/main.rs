use std::{
  path::{Path, PathBuf},
  process::Command,
};

use anyhow::{Context, Result, anyhow, bail};
use wasmtime::{
  Config, Engine, Store,
  component::{Component, Linker},
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

mod bindings;

use bindings::a::b::temperature_types;

const GUEST_TARGET: &str = "wasm32-wasip2";
const GUEST_WASM: &str = "wit_bindgen_example.wasm";

struct HostState {
  current_fahrenheit: f32,
  temperature_read_count: u32,
  conversion_count: u32,
  last_fahrenheit: Option<f32>,
  last_celsius: Option<f32>,
}

impl HostState {
  fn to_component_state(&self) -> temperature_types::HostState {
    temperature_types::HostState {
      current_fahrenheit: self.current_fahrenheit,
      temperature_read_count: self.temperature_read_count,
      conversion_count: self.conversion_count,
      last_fahrenheit: self.last_fahrenheit,
      last_celsius: self.last_celsius,
    }
  }

  fn save_component_state(&mut self, state: temperature_types::HostState) {
    self.current_fahrenheit = state.current_fahrenheit;
    self.temperature_read_count = state.temperature_read_count;
    self.conversion_count = state.conversion_count;
    self.last_fahrenheit = state.last_fahrenheit;
    self.last_celsius = state.last_celsius;
  }
}

struct StoreState {
  host: HostState,
  wasi: WasiCtx,
  table: ResourceTable,
}

impl WasiView for StoreState {
  fn ctx(&mut self) -> WasiCtxView<'_> {
    WasiCtxView {
      ctx: &mut self.wasi,
      table: &mut self.table,
    }
  }
}

fn main() -> Result<()> {
  let guest_component = ensure_guest_component()?;

  let mut config = Config::new();
  config.wasm_component_model(true);
  let engine = Engine::new(&config)?;
  let component = Component::from_file(&engine, &guest_component).map_err(|err| {
    anyhow!(
      "failed to load guest component {}: {err}",
      guest_component.display()
    )
  })?;

  let mut linker = Linker::new(&engine);
  wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;

  let current_fahrenheit = 72.0;
  let mut store = Store::new(
    &engine,
    StoreState {
      host: HostState {
        current_fahrenheit,
        temperature_read_count: 0,
        conversion_count: 0,
        last_fahrenheit: None,
        last_celsius: None,
      },
      wasi: WasiCtxBuilder::new().build(),
      table: ResourceTable::new(),
    },
  );
  let instance = bindings::TheWorld::instantiate(&mut store, &component, &linker)?;
  let component_state = store.data().host.to_component_state();
  let temperature_result = instance
    .temperature_service()
    .call_calculate_celsius(&mut store, component_state)?;
  store
    .data_mut()
    .host
    .save_component_state(temperature_result.state);
  let in_celsius = temperature_result.celsius;
  let host_state = &store.data().host;

  println!("current temp in fahrenheit is {current_fahrenheit}");
  println!("current temp in celsius is {}", in_celsius.degrees);
  println!(
    "temperature read count saved in host state: {}",
    host_state.temperature_read_count
  );
  println!(
    "conversion count saved in host state: {}",
    host_state.conversion_count
  );
  if let (Some(fahrenheit), Some(celsius)) = (host_state.last_fahrenheit, host_state.last_celsius) {
    println!("last wasm-side conversion saved in host state was {fahrenheit}F -> {celsius}C");
  }

  Ok(())
}

fn ensure_guest_component() -> Result<PathBuf> {
  let package_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let workspace_dir = package_dir
    .parent()
    .context("failed to determine workspace root for wit_bindgen_example")?;
  let guest_target_dir = workspace_dir.join("target").join("wit-bindgen-guest");
  let guest_component = guest_target_dir
    .join(GUEST_TARGET)
    .join("debug")
    .join(GUEST_WASM);

  let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
  let output = Command::new(cargo)
    .current_dir(package_dir)
    .arg("build")
    .arg("--lib")
    .arg("--target")
    .arg(GUEST_TARGET)
    .arg("--target-dir")
    .arg(&guest_target_dir)
    .output()
    .context("failed to invoke cargo to build the wit-bindgen guest component")?;

  if !output.status.success() {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.contains("the `wasm32-wasip2` target may not be installed")
      || stderr.contains("can't find crate for `core`")
    {
      bail!(
        "failed to build `{GUEST_WASM}` for `{GUEST_TARGET}`.\ninstall the target first with \
         `rustup target add {GUEST_TARGET}` and rerun `cargo run`.\n\ncargo stderr:\n{stderr}"
      );
    }

    bail!(
      "failed to build the guest component before launching the host example.\nmanifest: \
       {}\nexpected output: {}\n\ncargo stdout:\n{stdout}\n\ncargo stderr:\n{stderr}",
      package_dir.join("Cargo.toml").display(),
      guest_component.display(),
    );
  }

  if !guest_component.is_file() {
    bail!(
      "cargo reported success, but the guest component was not produced at {}",
      guest_component.display(),
    );
  }

  Ok(guest_component)
}

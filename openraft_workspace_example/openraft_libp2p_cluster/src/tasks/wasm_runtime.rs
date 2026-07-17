//! Runtime-agnostic WASM execution substrate for tasks.
//!
//! Two pieces, mirroring docker's split between *engine* and *image store*:
//!
//! - [`WasmRuntime`] — the generic execution trait. The worker pipeline only talks to `&'static dyn
//!   WasmRuntime`; today the sole implementation is [`WasmtimeRuntime`] (WASI 0.2 component runtime
//!   with fuel metering, moved here from `handlers.rs`). Supporting another engine (e.g. wasmer) =
//!   implement the trait + add one arm in [`runtime_by_name`] — no change to handlers or the worker
//!   loop.
//! - [`WasmModuleStore`] — the docker-like "code as file" side: a directory of `.wasm`/`.wat`
//!   module files (images). A task payload can carry just a `module_file` reference (+ optional
//!   sha256 digest pin) instead of embedding the whole module in the raft log.
//!
//! The runtime is selected once per process via the `WASM_RUNTIME` env var
//! (default `wasmtime`); the store directory via `WASM_MODULES_DIR`
//! (default `wasm_modules`, resolved against the worker's working dir).

use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

/// Env var selecting the execution engine (`wasmtime`).
pub const WASM_RUNTIME_ENV: &str = "WASM_RUNTIME";
/// Env var pointing at the module store directory.
pub const WASM_MODULES_DIR_ENV: &str = "WASM_MODULES_DIR";
/// Default module store directory (relative to the worker's working dir).
pub const WASM_MODULES_DIR_DEFAULT: &str = "wasm_modules";

/// Fuel budget per execution. Bounds runaway guests deterministically
/// (an infinite loop traps instead of eating the 30s execution timeout);
/// generous enough for demo workloads (~hundreds of millions of ops).
pub const WASM_FUEL_LIMIT: u64 = 500_000_000;
/// Captured-stdout cap; a guest writing past it traps (bounded results).
const WASM_STDOUT_CAPACITY: usize = 64 * 1024;

/// Everything an execution needs besides the module bytes: guest argv[1..],
/// environment, and the fuel budget (`None` = unmetered, for engines
/// without deterministic metering).
#[derive(Debug, Clone, Default)]
pub struct WasmInvocation {
  pub args: Vec<String>,
  pub env: Vec<(String, String)>,
  pub fuel_limit: Option<u64>,
}

/// What an execution produced: captured stdout (the task result) and fuel
/// burned (`None` when the engine does not meter).
#[derive(Debug, Clone)]
pub struct WasmOutcome {
  pub stdout: String,
  pub fuel_used: Option<u64>,
}

/// The generic execution engine interface (dyn-safe, sync — callers run it
/// on the blocking pool). `source` is any accepted module form: WAT text,
/// a core wasm binary, or a component binary; each engine normalizes
/// internally and is expected to cache compilations by content hash.
pub trait WasmRuntime: Send + Sync {
  /// Registry name (`wasmtime`, later maybe `wasmer`).
  fn name(&self) -> &'static str;

  /// Run `source` as a WASI command with `invocation`; non-zero guest exit
  /// or a trap is an `Err` (routed to the task retry path by the caller).
  fn execute(&self, source: &[u8], invocation: WasmInvocation) -> Result<WasmOutcome, String>;
}

/// Engine names [`runtime_by_name`] understands.
pub const KNOWN_RUNTIMES: &[&str] = &["wasmtime"];

/// Resolve an engine by registry name. Adding wasmer later = one arm here.
pub fn runtime_by_name(name: &str) -> Result<&'static dyn WasmRuntime, String> {
  match name {
    "wasmtime" => Ok(&WasmtimeRuntime),
    other => Err(format!(
      "unknown wasm runtime {other:?} (known: {})",
      KNOWN_RUNTIMES.join(", ")
    )),
  }
}

/// The process-wide engine, chosen by `WASM_RUNTIME` (default `wasmtime`).
pub fn selected_runtime() -> Result<&'static dyn WasmRuntime, String> {
  match std::env::var(WASM_RUNTIME_ENV) {
    Ok(name) => runtime_by_name(&name),
    Err(_) => runtime_by_name("wasmtime"),
  }
}

/// Content hash of a module source — the cache key and the digest that
/// `module_sha256` pins (docker image digest analogue).
pub fn module_hash(source: &[u8]) -> String {
  let digest = Sha256::digest(source);
  let mut hex = String::with_capacity(digest.len() * 2);
  for byte in digest {
    use std::fmt::Write as _;
    let _ = write!(hex, "{byte:02x}");
  }
  hex
}

// ---------------------------------------------------------------------------
// Module store: docker-like "code as file"
// ---------------------------------------------------------------------------

/// A directory of wasm module files, referenced from task payloads by bare
/// name (like a local docker image store). Lookup tries the name verbatim,
/// then `<name>.wasm`, then `<name>.wat`.
pub struct WasmModuleStore {
  dir: PathBuf,
}

impl WasmModuleStore {
  pub fn new(dir: impl Into<PathBuf>) -> Self {
    Self { dir: dir.into() }
  }

  /// The store every worker uses: `WASM_MODULES_DIR` or `wasm_modules`.
  /// Read per call so tests (and operators pointing a worker at a new
  /// store) never fight a cached value.
  pub fn from_env() -> Self {
    Self::new(
      std::env::var(WASM_MODULES_DIR_ENV).unwrap_or_else(|_| WASM_MODULES_DIR_DEFAULT.to_string()),
    )
  }

  pub fn dir(&self) -> &Path {
    &self.dir
  }

  /// Load a module's bytes by store name. The name must be a bare file
  /// name — path separators and `..` are rejected so a payload arriving
  /// over the network can never read outside the store directory.
  pub fn load(&self, name: &str) -> Result<Vec<u8>, String> {
    if name.is_empty()
      || name == ".."
      || name.starts_with('.')
      || name.contains(['/', '\\'])
      || name.contains("..")
    {
      return Err(format!(
        "invalid module_file {name:?}: must be a bare file name inside the module store"
      ));
    }

    let candidates = [
      self.dir.join(name),
      self.dir.join(format!("{name}.wasm")),
      self.dir.join(format!("{name}.wat")),
    ];
    for candidate in &candidates {
      if candidate.is_file() {
        return std::fs::read(candidate)
          .map_err(|err| format!("read wasm module {}: {err}", candidate.display()));
      }
    }
    Err(format!(
      "wasm module {name:?} not found in store {} (tried {name}, {name}.wasm, {name}.wat)",
      self.dir.display()
    ))
  }
}

// ---------------------------------------------------------------------------
// wasmtime implementation (WASI 0.2 component runtime)
// ---------------------------------------------------------------------------

/// The wasmtime engine: WASI 0.2 (preview 2) component runtime
/// (`wasmtime_wasi::p2`, `wasi:cli/run` command world). Native p2
/// components run directly; classic p1 core modules (including hand-written
/// WAT importing `wasi_snapshot_preview1`) are wrapped with wasmtime's
/// official preview1 command adapter at load time, so every guest flows
/// through the single p2 path. Hardened with a fuel budget (from
/// `wasmtime_workspace_example/wasmtime_sandbox_limits`): stdout is
/// captured (bounded) as the task result and a fuel limit terminates
/// runaway modules deterministically.
pub struct WasmtimeRuntime;

/// One shared engine (fuel metering on); components are compiled once per
/// content hash and cached, so repeated tasks reuse the compilation.
fn wasm_engine() -> Result<&'static wasmtime::Engine, String> {
  static ENGINE: std::sync::OnceLock<Result<wasmtime::Engine, String>> = std::sync::OnceLock::new();
  ENGINE
    .get_or_init(|| {
      let mut config = wasmtime::Config::new();
      config.consume_fuel(true);
      wasmtime::Engine::new(&config).map_err(|err| format!("create wasm engine: {err}"))
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// Per-execution store state for the p2 WASI host (the `WasiView` pattern
/// from `wasmtime_workspace_example/kameo_wasmtime_hot_upgrade`).
struct WasmHostState {
  wasi: wasmtime_wasi::WasiCtx,
  table: wasmtime_wasi::ResourceTable,
}

impl wasmtime_wasi::WasiView for WasmHostState {
  fn ctx(&mut self) -> wasmtime_wasi::WasiCtxView<'_> {
    wasmtime_wasi::WasiCtxView {
      ctx: &mut self.wasi,
      table: &mut self.table,
    }
  }
}

/// One shared component linker with the full p2 WASI host wired in; shared
/// across executions (each execution gets its own Store).
fn wasm_linker() -> Result<&'static wasmtime::component::Linker<WasmHostState>, String> {
  static LINKER: std::sync::OnceLock<Result<wasmtime::component::Linker<WasmHostState>, String>> =
    std::sync::OnceLock::new();
  LINKER
    .get_or_init(|| {
      let engine = wasm_engine()?;
      let mut linker = wasmtime::component::Linker::new(engine);
      wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
        .map_err(|err| format!("link p2 wasi: {err}"))?;
      Ok(linker)
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// True when the binary is a component (layer field 1) rather than a core
/// module (layer 0). Header: `\0asm` + version u16 + layer u16.
pub(crate) fn is_component_binary(bytes: &[u8]) -> bool {
  bytes.len() >= 8 && bytes[0 .. 4] == *b"\0asm" && bytes[6 .. 8] == [0x01, 0x00]
}

/// Normalize a WASM source (WAT text, core-module binary, or component
/// binary) into p2 component bytes: components pass through; p1 core
/// modules are wrapped with wasmtime's official preview1 command adapter
/// so they run on the same p2 runtime.
pub fn componentize(source: &[u8]) -> Result<Vec<u8>, String> {
  // `wat::parse_bytes` is a no-op for binaries and assembles WAT text
  // (both `(module ...)` and `(component ...)` syntax).
  let binary = wat::parse_bytes(source)
    .map_err(|err| format!("parse wasm source: {err}"))?
    .into_owned();
  if is_component_binary(&binary) {
    return Ok(binary);
  }
  wit_component::ComponentEncoder::default()
    .module(&binary)
    .map_err(|err| format!("read core wasm module: {err}"))?
    .adapter(
      "wasi_snapshot_preview1",
      wasi_preview1_component_adapter_provider::WASI_SNAPSHOT_PREVIEW1_COMMAND_ADAPTER,
    )
    .map_err(|err| format!("attach preview1 adapter: {err}"))?
    .validate(true)
    .encode()
    .map_err(|err| format!("componentize p1 module: {err}"))
}

/// A compiled command component — the expensive part, cached by content
/// hash. Command components are one-shot (the Store is consumed after one
/// `run`), so Store/instance are created per execution.
struct CompiledWasm {
  component: wasmtime::component::Component,
}

impl CompiledWasm {
  /// Load a WASM source and compile it into a runnable command component.
  /// It is NOT instantiated here — that happens per-execution.
  fn load(engine: &wasmtime::Engine, source: &[u8]) -> Result<Self, String> {
    let component_bytes = componentize(source)?;
    let component = wasmtime::component::Component::new(engine, &component_bytes)
      .map_err(|err| format!("compile wasm component: {err}"))?;
    Ok(Self { component })
  }
}

/// Cache capacity: compiled modules are a few hundred KB each; a stream of
/// distinct modules must not grow worker memory without bound.
const WASM_COMPILE_CACHE_CAP: usize = 64;

/// Bounded FIFO cache of compiled components, keyed by module content
/// hash. Same module bytes → same hash → reuse compiled component; when
/// full, the oldest entry is evicted.
#[derive(Default)]
struct WasmCompileCache {
  compiled: std::collections::HashMap<String, CompiledWasm>,
  order: std::collections::VecDeque<String>,
}

impl WasmCompileCache {
  fn get(&self, hash: &str) -> Option<wasmtime::component::Component> {
    self.compiled.get(hash).map(|c| c.component.clone())
  }

  fn insert(&mut self, hash: String, compiled: CompiledWasm) {
    if self.compiled.contains_key(&hash) {
      return; // lost a compile race; keep the existing entry
    }
    while self.compiled.len() >= WASM_COMPILE_CACHE_CAP {
      let Some(evicted) = self.order.pop_front() else {
        break;
      };
      self.compiled.remove(&evicted);
    }
    self.order.push_back(hash.clone());
    self.compiled.insert(hash, compiled);
  }
}

fn wasm_compile_cache() -> &'static std::sync::Mutex<WasmCompileCache> {
  static CACHE: std::sync::OnceLock<std::sync::Mutex<WasmCompileCache>> =
    std::sync::OnceLock::new();
  CACHE.get_or_init(Default::default)
}

impl WasmRuntime for WasmtimeRuntime {
  fn name(&self) -> &'static str {
    "wasmtime"
  }

  fn execute(&self, source: &[u8], invocation: WasmInvocation) -> Result<WasmOutcome, String> {
    let engine = wasm_engine()?;
    let hash = module_hash(source);

    // Get or compile: the first execution of a given module compiles it
    // (componentize + cranelift codegen — the expensive part); subsequent
    // executions with the same bytes reuse the compiled component
    // (Arc-backed, so clone is cheap). Compilation happens OUTSIDE the
    // cache lock so a slow compile never serializes other wasm executions;
    // a concurrent duplicate compile just loses the insert race.
    let cached = wasm_compile_cache()
      .lock()
      .map_err(|_| "wasm compile cache poisoned".to_string())?
      .get(&hash);
    let component = match cached {
      Some(component) => {
        tracing::debug!(module_hash = %hash[.. 12], "reusing cached compiled wasm component");
        component
      }
      None => {
        tracing::debug!(module_hash = %hash[.. 12], "compiling and caching new wasm component");
        let compiled =
          CompiledWasm::load(engine, source).map_err(|err| format!("load wasm module: {err}"))?;
        let component = compiled.component.clone();
        wasm_compile_cache()
          .lock()
          .map_err(|_| "wasm compile cache poisoned".to_string())?
          .insert(hash.clone(), compiled);
        component
      }
    };

    // Fresh p2 WASI context + Store per execution; the linker (with the
    // full WASI host) and the compiled component are shared.
    let stdout = wasmtime_wasi::p2::pipe::MemoryOutputPipe::new(WASM_STDOUT_CAPACITY);
    let mut argv = vec!["task.wasm".to_string()];
    argv.extend(invocation.args);
    let wasi = wasmtime_wasi::WasiCtx::builder()
      .stdout(stdout.clone())
      .args(&argv)
      .envs(&invocation.env)
      .build();

    let mut store = wasmtime::Store::new(
      engine,
      WasmHostState {
        wasi,
        table: wasmtime_wasi::ResourceTable::new(),
      },
    );
    let fuel_limit = invocation.fuel_limit.unwrap_or(WASM_FUEL_LIMIT);
    store
      .set_fuel(fuel_limit)
      .map_err(|err| format!("set wasm fuel: {err}"))?;

    let command = wasmtime_wasi::p2::bindings::sync::Command::instantiate(
      &mut store,
      &component,
      wasm_linker()?,
    )
    .map_err(|err| format!("instantiate wasm component: {err}"))?;

    match command.wasi_cli_run().call_run(&mut store) {
      Ok(Ok(())) => {}
      Ok(Err(())) => return Err("wasm command run returned failure".to_string()),
      Err(err) => {
        // `exit(n)` surfaces as an I32Exit in the error chain; 0 is
        // success, anything else is a failure.
        let exit = err
          .chain()
          .find_map(|cause| cause.downcast_ref::<wasmtime_wasi::I32Exit>());
        match exit {
          Some(wasmtime_wasi::I32Exit(0)) => {}
          Some(wasmtime_wasi::I32Exit(code)) => {
            return Err(format!("wasm module exited with code {code}"));
          }
          None => return Err(format!("wasm execution trapped: {err}")),
        }
      }
    }

    let fuel_used = fuel_limit.saturating_sub(store.get_fuel().unwrap_or(0));
    let output = String::from_utf8_lossy(&stdout.contents()).into_owned();
    Ok(WasmOutcome {
      stdout: output,
      fuel_used: Some(fuel_used),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn runtime_registry_resolves_wasmtime_and_rejects_unknown() {
    assert_eq!(runtime_by_name("wasmtime").unwrap().name(), "wasmtime");
    let err = runtime_by_name("wasmer").map(|r| r.name()).unwrap_err();
    assert!(err.contains("wasmer"), "unexpected error: {err}");
    assert!(
      err.contains("wasmtime"),
      "should list known runtimes: {err}"
    );
  }

  #[test]
  fn module_store_resolves_bare_name_and_extensions() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.wat"), "(module)").unwrap();
    std::fs::write(dir.path().join("bin.wasm"), b"\0asm").unwrap();
    let store = WasmModuleStore::new(dir.path());

    assert_eq!(store.load("hello").unwrap(), b"(module)");
    assert_eq!(store.load("hello.wat").unwrap(), b"(module)");
    assert_eq!(store.load("bin").unwrap(), b"\0asm");
    let err = store.load("missing").unwrap_err();
    assert!(err.contains("not found"), "unexpected error: {err}");
  }

  #[test]
  fn module_store_rejects_path_traversal() {
    let store = WasmModuleStore::new("/tmp/does-not-matter");
    for name in ["../etc/passwd", "..", "a/b", "a\\b", ".hidden", ""] {
      let err = store.load(name).unwrap_err();
      assert!(
        err.contains("bare file name"),
        "{name:?} should be rejected, got: {err}"
      );
    }
  }

  /// Executing through the trait object (the worker's view of the engine).
  #[test]
  fn wasmtime_runtime_executes_via_trait_object() {
    let runtime: &dyn WasmRuntime = runtime_by_name("wasmtime").unwrap();
    // Minimal WASI module writing "ok\n" to stdout via fd_write.
    let wat = r#"
      (module
        (import "wasi_snapshot_preview1" "fd_write"
          (func $fd_write (param i32 i32 i32 i32) (result i32)))
        (memory 1)
        (export "memory" (memory 0))
        (data (i32.const 8) "ok\n")
        (func (export "_start")
          (i32.store (i32.const 0) (i32.const 8))
          (i32.store (i32.const 4) (i32.const 3))
          (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 32)))))
    "#;
    let outcome = runtime
      .execute(wat.as_bytes(), WasmInvocation::default())
      .unwrap();
    assert_eq!(outcome.stdout, "ok\n");
    assert!(outcome.fuel_used.unwrap() > 0);
  }
}

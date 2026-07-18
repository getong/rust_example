//! Runtime-agnostic WASM execution substrate for tasks.
//!
//! Two pieces, mirroring docker's split between *engine* and *image store*:
//!
//! - [`WasmRuntime`] — the generic execution trait. The worker pipeline only talks to `&'static dyn
//!   WasmRuntime`; today the sole implementation is [`WasmtimeRuntime`] (WASI 0.2 component runtime
//!   with fuel metering). Supporting another engine (e.g. wasmer) = implement the trait + add one
//!   arm in [`runtime_by_name`] — no change to handlers or the worker loop.
//! - [`WasmModuleStore`] — the docker-like "code as file" side: a directory of `.wasm`/`.wat`
//!   module files (images). A task payload can carry just a `module_file` reference (+ optional
//!   sha256 digest pin) instead of embedding the whole module in the raft log.
//!
//! Hardening and performance follow `docs/first-wasm-optimization-review.md`:
//!
//! - Sandbox (§2): fuel metering + [`ExecutionLimiter`] (memory/table caps) + epoch interruption as
//!   a wall-clock backstop, so a guest that defeats fuel accounting still cannot pin an executor
//!   thread forever.
//! - Scheduling (§1): executions run on a dedicated fixed-size thread pool ([`execute_pooled`]),
//!   isolated from tokio's shared blocking pool, with execution/fuel metrics.
//! - Compile cache (§5/§6): componentize+compile results are cached by content hash behind an
//!   `RwLock` (reads never contend), LRU-evicted by entry count AND total byte budget;
//!   [`warm_compile_cache`] precompiles the module store at worker startup.
//! - Typed guests (§3/§4/§7): components exporting the `cluster:task/runner` WIT interface
//!   (`wit/task-executor.wit`) exchange typed records instead of stdout scraping, may call back
//!   into host functions (`cluster:task/host`), and batch executions reuse one instance
//!   ([`WasmRuntime::execute_batch`]). Classic WASI CLI guests keep working unchanged.
//!
//! The runtime is selected once per process via the `WASM_RUNTIME` env var
//! (default `wasmtime`); the store directory via `WASM_MODULES_DIR`
//! (default `wasm_modules`, resolved against the worker's working dir).

use std::{
  path::{Path, PathBuf},
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};

use sha2::{Digest as _, Sha256};

/// Env var selecting the execution engine (`wasmtime`).
pub const WASM_RUNTIME_ENV: &str = "WASM_RUNTIME";
/// Env var pointing at the module store directory.
pub const WASM_MODULES_DIR_ENV: &str = "WASM_MODULES_DIR";
/// Default module store directory (relative to the worker's working dir).
pub const WASM_MODULES_DIR_DEFAULT: &str = "wasm_modules";
/// Env var sizing the dedicated wasm executor pool (`WASM_EXECUTOR_THREADS`).
pub const WASM_EXECUTOR_THREADS_ENV: &str = "WASM_EXECUTOR_THREADS";

/// Fuel budget per execution. Bounds runaway guests deterministically
/// (an infinite loop traps instead of eating the 30s execution timeout);
/// generous enough for demo workloads (~hundreds of millions of ops).
pub const WASM_FUEL_LIMIT: u64 = 500_000_000;
/// Captured-stdout cap; a guest writing past it traps (bounded results).
const WASM_STDOUT_CAPACITY: usize = 64 * 1024;

/// Per-execution linear-memory cap (all memories in the Store together
/// stay under this via [`ExecutionLimiter`]); prevents a guest OOMing the
/// worker regardless of fuel.
pub const WASM_MEMORY_LIMIT_BYTES: usize = 64 * 1024 * 1024;
/// Per-table element cap.
pub const WASM_TABLE_LIMIT_ELEMENTS: usize = 10_000;
/// Core instances allowed per Store. A p2 component expands to several
/// core instances (module + adapter shims), so this is a hard backstop,
/// not "one instance".
const WASM_MAX_INSTANCES: usize = 16;
/// Linear memories / tables allowed per Store (component + shims).
const WASM_MAX_MEMORIES: usize = 8;
const WASM_MAX_TABLES: usize = 8;

/// Wall-clock backstop when the invocation does not set one: even if fuel
/// accounting is defeated (host-call heavy loops burn little fuel), the
/// epoch deadline interrupts the guest and releases the executor thread.
pub const WASM_WALL_CLOCK_BACKSTOP: Duration = Duration::from_secs(60);
/// How often the background ticker advances the engine epoch; one tick is
/// the granularity of every wall-clock deadline.
const WASM_EPOCH_TICK: Duration = Duration::from_millis(100);

/// Warn when one execution burns more than this share of its fuel budget —
/// the operator signal (review §1) that a workload is about to start
/// failing with fuel exhaustion.
const WASM_FUEL_WARN_RATIO: f64 = 0.8;

/// Everything an execution needs besides the module bytes: guest argv[1..],
/// environment, the fuel budget (`None` = engine default), and an optional
/// wall-clock cap (`None` = [`WASM_WALL_CLOCK_BACKSTOP`]).
#[derive(Debug, Clone, Default)]
pub struct WasmInvocation {
  pub args: Vec<String>,
  pub env: Vec<(String, String)>,
  pub fuel_limit: Option<u64>,
  pub wall_clock_limit: Option<Duration>,
  /// Task record id, surfaced to typed guests via `host.task-id`.
  pub task_id: Option<String>,
  /// Read-only key/value config, surfaced via `host.config-get`.
  pub config: Vec<(String, String)>,
}

/// What an execution produced: captured stdout (the task result), fuel
/// burned (`None` when the engine does not meter), and the optional typed
/// JSON result returned by `cluster:task/runner` guests.
#[derive(Debug, Clone)]
pub struct WasmOutcome {
  pub stdout: String,
  pub fuel_used: Option<u64>,
  pub structured: Option<String>,
}

/// The generic execution engine interface (dyn-safe, sync — callers run it
/// on the dedicated executor pool via [`execute_pooled`]). `source` is any
/// accepted module form: WAT text, a core wasm binary, or a component
/// binary; each engine normalizes internally and is expected to cache
/// compilations by content hash.
pub trait WasmRuntime: Send + Sync {
  /// Registry name (`wasmtime`, later maybe `wasmer`).
  fn name(&self) -> &'static str;

  /// Run `source` as one task with `invocation`; guest failure, traps and
  /// fuel/deadline exhaustion are `Err` (routed to the task retry path).
  fn execute(&self, source: &[u8], invocation: WasmInvocation) -> Result<WasmOutcome, String>;

  /// Run several invocations of the SAME module (review §4). The default
  /// simply loops [`WasmRuntime::execute`]; engines may override to
  /// amortize instantiation (wasmtime reuses one Store + instance for
  /// typed `cluster:task/runner` guests).
  fn execute_batch(
    &self,
    source: &[u8],
    invocations: Vec<WasmInvocation>,
  ) -> Vec<Result<WasmOutcome, String>> {
    invocations
      .into_iter()
      .map(|invocation| self.execute(source, invocation))
      .collect()
  }

  /// Compile `source` into the engine's cache without running it (cache
  /// warm-up, review §5). Default: no-op for engines without a cache.
  fn prepare(&self, _source: &[u8]) -> Result<(), String> {
    Ok(())
  }
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

  /// Like [`Self::from_env`] but the env lookup is resolved once per
  /// process. For hot paths that construct a store per call (chunk
  /// serving, inventory announcements, sync rounds): the env var cannot
  /// change mid-process, so re-reading it there is pure overhead. Tests
  /// that set `WASM_MODULES_DIR` at runtime must keep using
  /// [`Self::from_env`].
  pub fn from_env_cached() -> Self {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    let dir = DIR.get_or_init(|| {
      PathBuf::from(
        std::env::var(WASM_MODULES_DIR_ENV)
          .unwrap_or_else(|_| WASM_MODULES_DIR_DEFAULT.to_string()),
      )
    });
    Self::new(dir.clone())
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

  /// File names of every `.wasm`/`.wat` module in the store (empty when
  /// the directory does not exist — an empty store is not an error).
  pub fn list(&self) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(&self.dir) else {
      return Vec::new();
    };
    let mut names: Vec<String> = entries
      .filter_map(|entry| entry.ok())
      .filter(|entry| entry.path().is_file())
      .filter_map(|entry| entry.file_name().into_string().ok())
      .filter(|name| name.ends_with(".wasm") || name.ends_with(".wat"))
      .collect();
    names.sort();
    names
  }
}

// ---------------------------------------------------------------------------
// Dedicated executor pool (review §1)
// ---------------------------------------------------------------------------

/// Fixed-size pool for wasm execution, isolated from tokio's shared
/// blocking pool: a burst of wasm tasks can no longer starve the blocking
/// threads used by file IO / DNS / other `spawn_blocking` users, and the
/// pool size caps concurrent guest memory (pool_size × memory limit).
fn wasm_executor_pool() -> &'static rayon::ThreadPool {
  static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
  POOL.get_or_init(|| {
    let threads = std::env::var(WASM_EXECUTOR_THREADS_ENV)
      .ok()
      .and_then(|raw| raw.parse::<usize>().ok())
      .filter(|n| *n > 0)
      .unwrap_or_else(|| {
        std::thread::available_parallelism()
          .map(|n| n.get().min(4))
          .unwrap_or(2)
      });
    rayon::ThreadPoolBuilder::new()
      .num_threads(threads)
      .thread_name(|i| format!("wasm-exec-{i}"))
      // A panicking execution must not take the process down; the oneshot
      // sender is dropped and the caller sees a clean error.
      .panic_handler(|panic| tracing::error!(?panic, "wasm executor thread panicked"))
      .build()
      .expect("build wasm executor pool")
  })
}

/// Execute on the dedicated pool and record execution metrics: duration,
/// outcome, fuel burned, and a warning when a task gets close to its fuel
/// budget (the review §1 "fuel alert threshold").
pub async fn execute_pooled(
  runtime: &'static dyn WasmRuntime,
  source: Vec<u8>,
  invocation: WasmInvocation,
) -> Result<WasmOutcome, String> {
  let fuel_budget = invocation.fuel_limit.unwrap_or(WASM_FUEL_LIMIT);
  // Structured execution span (production observability): carries the
  // runtime + task id, is entered on the executor thread so guest host
  // calls (log, progress) nest under it, and records execution_time /
  // fuel_used once the outcome is known.
  let span = tracing::info_span!(
    "wasm_execution",
    runtime = runtime.name(),
    task_id = invocation.task_id.as_deref().unwrap_or(""),
    execution_time_ms = tracing::field::Empty,
    fuel_used = tracing::field::Empty,
    outcome = tracing::field::Empty,
  );
  let (tx, rx) = tokio::sync::oneshot::channel();
  let executor_span = span.clone();
  wasm_executor_pool().spawn(move || {
    let _entered = executor_span.enter();
    let started = std::time::Instant::now();
    let outcome = runtime.execute(&source, invocation);
    let _ = tx.send((outcome, started.elapsed()));
  });
  let (outcome, elapsed) = rx
    .await
    .map_err(|_| "wasm executor dropped the result (execution panicked?)".to_string())?;

  let runtime_label = runtime.name();
  span.record("execution_time_ms", elapsed.as_millis() as u64);
  span.record("outcome", if outcome.is_ok() { "ok" } else { "err" });
  metrics::histogram!("wasm_execution_duration_seconds", "runtime" => runtime_label)
    .record(elapsed.as_secs_f64());
  match &outcome {
    Ok(done) => {
      metrics::counter!("wasm_executions_total", "runtime" => runtime_label, "outcome" => "ok")
        .increment(1);
      if let Some(fuel_used) = done.fuel_used {
        span.record("fuel_used", fuel_used);
        metrics::histogram!("wasm_fuel_used", "runtime" => runtime_label).record(fuel_used as f64);
        if fuel_budget > 0 && fuel_used as f64 / fuel_budget as f64 > WASM_FUEL_WARN_RATIO {
          tracing::warn!(
            fuel_used,
            fuel_budget,
            "wasm execution used more than {}% of its fuel budget",
            (WASM_FUEL_WARN_RATIO * 100.0) as u32
          );
        }
      }
    }
    Err(error) => {
      metrics::counter!("wasm_executions_total", "runtime" => runtime_label, "outcome" => "err")
        .increment(1);
      if error.contains("out of fuel") {
        metrics::counter!("wasm_fuel_exhausted_total", "runtime" => runtime_label).increment(1);
      }
    }
  }
  outcome
}

/// Precompile every module in the store on the executor pool (review §5
/// cache warm-up): the first task referencing a deployed `module_file`
/// skips its cold-compile latency. Fire-and-forget; failures only log.
pub fn warm_compile_cache() {
  let runtime = match selected_runtime() {
    Ok(runtime) => runtime,
    Err(err) => {
      tracing::warn!(error = %err, "skipping wasm cache warm-up");
      return;
    }
  };
  let store = WasmModuleStore::from_env_cached();
  let names = store.list();
  if names.is_empty() {
    return;
  }
  tracing::info!(
    modules = names.len(),
    dir = %store.dir().display(),
    "warming wasm compile cache from module store"
  );
  for name in names {
    let dir = store.dir().to_path_buf();
    wasm_executor_pool().spawn(move || {
      let store = WasmModuleStore::new(dir);
      match store.load(&name).and_then(|bytes| runtime.prepare(&bytes)) {
        Ok(()) => tracing::debug!(module = %name, "warmed wasm compile cache"),
        Err(err) => tracing::warn!(module = %name, error = %err, "wasm cache warm-up failed"),
      }
    });
  }
}

// ---------------------------------------------------------------------------
// wasmtime implementation (WASI 0.2 component runtime)
// ---------------------------------------------------------------------------

/// The wasmtime engine: WASI 0.2 (preview 2) component runtime
/// (`wasmtime_wasi::p2`). Guests exporting the typed
/// `cluster:task/runner` interface run through WIT records; everything
/// else runs as a `wasi:cli/run` command. Native p2 components run
/// directly; classic p1 core modules (including hand-written WAT importing
/// `wasi_snapshot_preview1`) are wrapped with wasmtime's official preview1
/// command adapter at load time (feature `p1-compat`). Hardened with fuel,
/// memory/table limits and an epoch wall-clock backstop (patterns from
/// `wasmtime_workspace_example/wasmtime_sandbox_limits`).
pub struct WasmtimeRuntime;

/// Host-side bindings for `wit/task-executor.wit` (typed guests, §3/§7).
mod typed {
  wasmtime::component::bindgen!({
    path: "wit/task-executor.wit",
    world: "task-executor",
  });
}

/// Stop signal for the epoch ticker thread (v2 review §6): the process
/// lifetime model never needs it, but embedders and tests can call
/// [`stop_epoch_ticker`] to let the thread exit instead of leaking it.
fn epoch_ticker_stop() -> &'static std::sync::atomic::AtomicBool {
  static STOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
  &STOP
}

/// Ask the background epoch ticker to exit after its next tick. NOTE:
/// running and future executions lose the wall-clock backstop (the fuel
/// budget still bounds them) — only call this on the way to shutdown.
pub fn stop_epoch_ticker() {
  epoch_ticker_stop().store(true, Ordering::Relaxed);
}

/// One shared engine (fuel metering + epoch interruption on); components
/// are compiled once per content hash and cached. The first use spawns the
/// epoch ticker thread that advances the wall clock for every Store.
fn wasm_engine() -> Result<&'static wasmtime::Engine, String> {
  static ENGINE: std::sync::OnceLock<Result<wasmtime::Engine, String>> = std::sync::OnceLock::new();
  ENGINE
    .get_or_init(|| {
      let mut config = wasmtime::Config::new();
      config.consume_fuel(true);
      config.epoch_interruption(true);
      // Explicit even though wasmtime v46 defaults it on (v2 review §1):
      // the component::{Component, Linker} usage below must not silently
      // break if a future wasmtime changes the default.
      config.wasm_component_model(true);
      let engine =
        wasmtime::Engine::new(&config).map_err(|err| format!("create wasm engine: {err}"))?;
      // Wall-clock backstop (review §2): fuel is deterministic but can be
      // gamed by host-call heavy guests; the epoch ticker guarantees every
      // execution is interrupted after its deadline regardless.
      let ticker = engine.clone();
      std::thread::Builder::new()
        .name("wasm-epoch-ticker".to_string())
        .spawn(move || {
          while !epoch_ticker_stop().load(Ordering::Relaxed) {
            std::thread::sleep(WASM_EPOCH_TICK);
            ticker.increment_epoch();
          }
          tracing::debug!("wasm epoch ticker stopped");
        })
        .map_err(|err| format!("spawn wasm epoch ticker: {err}"))?;
      Ok(engine)
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// Ticks of [`WASM_EPOCH_TICK`] covering `limit` (rounded up, minimum 1).
fn epoch_deadline_ticks(limit: Duration) -> u64 {
  limit
    .as_millis()
    .div_ceil(WASM_EPOCH_TICK.as_millis())
    .max(1) as u64
}

/// Per-Store resource limiter (review §2, pattern from
/// `wasmtime_sandbox_limits`): denies memory growth past
/// [`WASM_MEMORY_LIMIT_BYTES`] and table growth past
/// [`WASM_TABLE_LIMIT_ELEMENTS`]. The guest sees `memory.grow`/`table.grow`
/// fail (-1), exactly like a resource-exhausted host.
struct ExecutionLimiter {
  memory_limit_bytes: usize,
  table_limit_elements: usize,
}

impl Default for ExecutionLimiter {
  fn default() -> Self {
    Self {
      memory_limit_bytes: WASM_MEMORY_LIMIT_BYTES,
      table_limit_elements: WASM_TABLE_LIMIT_ELEMENTS,
    }
  }
}

impl wasmtime::ResourceLimiter for ExecutionLimiter {
  fn memory_growing(
    &mut self,
    current: usize,
    desired: usize,
    _maximum: Option<usize>,
  ) -> wasmtime::Result<bool> {
    let allowed = desired <= self.memory_limit_bytes;
    if !allowed {
      tracing::warn!(
        current,
        desired,
        limit = self.memory_limit_bytes,
        "wasm memory growth denied by limiter"
      );
    }
    Ok(allowed)
  }

  fn table_growing(
    &mut self,
    current: usize,
    desired: usize,
    _maximum: Option<usize>,
  ) -> wasmtime::Result<bool> {
    let allowed = desired <= self.table_limit_elements;
    if !allowed {
      tracing::warn!(
        current,
        desired,
        limit = self.table_limit_elements,
        "wasm table growth denied by limiter"
      );
    }
    Ok(allowed)
  }

  fn instances(&self) -> usize {
    WASM_MAX_INSTANCES
  }

  fn tables(&self) -> usize {
    WASM_MAX_TABLES
  }

  fn memories(&self) -> usize {
    WASM_MAX_MEMORIES
  }
}

/// Per-execution store state: p2 WASI host (the `WasiView` pattern from
/// `wasmtime_workspace_example/kameo_wasmtime_hot_upgrade`) + the resource
/// limiter + the data backing the `cluster:task/host` import.
struct WasmHostState {
  wasi: wasmtime_wasi::WasiCtx,
  table: wasmtime_wasi::ResourceTable,
  limiter: ExecutionLimiter,
  /// Task record id surfaced to the guest via `host.task-id`.
  task_id: Option<String>,
  /// Task-supplied read-only configuration for `host.config-get`.
  config: std::collections::HashMap<String, String>,
  /// When the epoch backstop will interrupt this execution
  /// (`host.wall-clock-remaining-ms`).
  deadline: std::time::Instant,
}

impl wasmtime_wasi::WasiView for WasmHostState {
  fn ctx(&mut self) -> wasmtime_wasi::WasiCtxView<'_> {
    wasmtime_wasi::WasiCtxView {
      ctx: &mut self.wasi,
      table: &mut self.table,
    }
  }
}

/// Process-wide read-only KV lookup backing `host.kv-get`:
/// `(group_id, key) -> value`. Registered once at startup by the
/// composition root (which owns the `GroupRegistry`) via
/// [`register_wasm_kv_reader`]; the wasm runtime itself stays free of any
/// raft/store dependency. Unregistered (client binaries, unit tests)
/// means every `kv-get` answers none.
pub type WasmKvReader = std::sync::Arc<dyn Fn(&str, &str) -> Option<String> + Send + Sync>;

static WASM_KV_READER: std::sync::OnceLock<WasmKvReader> = std::sync::OnceLock::new();

/// Install the KV read hook for `cluster:task/host.kv-get`. First
/// registration wins; later calls are ignored (one composition root per
/// process).
pub fn register_wasm_kv_reader(reader: WasmKvReader) {
  let _ = WASM_KV_READER.set(reader);
}

/// The state-backed half of the `cluster:task/host` interface (v2 review
/// §4/§5), factored into a trait so tests can mock it (§11) and drive
/// guest-mirroring logic without a wasm instance. The fuel / wall-clock
/// queries are NOT here: they need the live Store handle and are
/// implemented directly in the [`add_task_host_to_linker`] closures.
#[cfg_attr(test, mockall::automock)]
pub trait TaskHostOps {
  /// Guest diagnostics through host tracing.
  fn log(&mut self, message: &str);
  /// Guest progress, surfaced via tracing + metrics.
  fn report_progress(&mut self, progress: u32, total: u32);
  /// The executing task's record id ("" outside a task).
  fn task_id(&self) -> String;
  /// Task-supplied read-only config lookup.
  fn config_get(&self, key: &str) -> Option<String>;
  /// Read-only replicated-KV lookup (none when no reader is registered).
  fn kv_get(&self, group: &str, key: &str) -> Option<String>;
}

impl TaskHostOps for WasmHostState {
  fn log(&mut self, message: &str) {
    tracing::info!(target: "wasm_guest", task_id = self.task_id.as_deref().unwrap_or(""), "{message}");
  }

  fn report_progress(&mut self, progress: u32, total: u32) {
    tracing::info!(
      target: "wasm_guest",
      task_id = self.task_id.as_deref().unwrap_or(""),
      progress,
      total,
      "guest progress"
    );
    metrics::counter!("wasm_guest_progress_reports_total").increment(1);
  }

  fn task_id(&self) -> String {
    self.task_id.clone().unwrap_or_default()
  }

  fn config_get(&self, key: &str) -> Option<String> {
    self.config.get(key).cloned()
  }

  fn kv_get(&self, group: &str, key: &str) -> Option<String> {
    let reader = WASM_KV_READER.get()?;
    let value = reader(group, key);
    metrics::counter!(
      "wasm_guest_kv_gets_total",
      "found" => if value.is_some() { "true" } else { "false" }
    )
    .increment(1);
    value
  }
}

/// Wire the `cluster:task/host` interface. Registered by hand with
/// `LinkerInstance::func_wrap` instead of bindgen's generated
/// `add_to_linker` because `fuel-remaining` / `wall-clock-remaining-ms`
/// need the live Store handle (fuel lives on the Store, not in the store
/// DATA), which only raw `func_wrap` closures receive.
fn add_task_host_to_linker(
  linker: &mut wasmtime::component::Linker<WasmHostState>,
) -> Result<(), String> {
  let mut host = linker
    .instance("cluster:task/host")
    .map_err(|err| format!("define cluster:task/host: {err}"))?;
  host
    .func_wrap(
      "log",
      |mut store: wasmtime::StoreContextMut<'_, WasmHostState>, (message,): (String,)| {
        store.data_mut().log(&message);
        Ok(())
      },
    )
    .map_err(|err| format!("link host.log: {err}"))?;
  host
    .func_wrap(
      "task-id",
      |store: wasmtime::StoreContextMut<'_, WasmHostState>, (): ()| Ok((store.data().task_id(),)),
    )
    .map_err(|err| format!("link host.task-id: {err}"))?;
  host
    .func_wrap(
      "fuel-remaining",
      |store: wasmtime::StoreContextMut<'_, WasmHostState>, (): ()| {
        Ok((store.get_fuel().unwrap_or(0),))
      },
    )
    .map_err(|err| format!("link host.fuel-remaining: {err}"))?;
  host
    .func_wrap(
      "wall-clock-remaining-ms",
      |store: wasmtime::StoreContextMut<'_, WasmHostState>, (): ()| {
        let remaining = store
          .data()
          .deadline
          .saturating_duration_since(std::time::Instant::now());
        Ok((remaining.as_millis() as i64,))
      },
    )
    .map_err(|err| format!("link host.wall-clock-remaining-ms: {err}"))?;
  host
    .func_wrap(
      "report-progress",
      |mut store: wasmtime::StoreContextMut<'_, WasmHostState>, (progress, total): (u32, u32)| {
        store.data_mut().report_progress(progress, total);
        Ok(())
      },
    )
    .map_err(|err| format!("link host.report-progress: {err}"))?;
  host
    .func_wrap(
      "config-get",
      |store: wasmtime::StoreContextMut<'_, WasmHostState>, (key,): (String,)| {
        Ok((store.data().config_get(&key),))
      },
    )
    .map_err(|err| format!("link host.config-get: {err}"))?;
  host
    .func_wrap(
      "kv-get",
      |store: wasmtime::StoreContextMut<'_, WasmHostState>, (group, key): (String, String)| {
        Ok((store.data().kv_get(&group, &key),))
      },
    )
    .map_err(|err| format!("link host.kv-get: {err}"))?;
  Ok(())
}

/// One shared component linker with the full p2 WASI host AND the
/// `cluster:task/host` functions wired in; shared across executions (each
/// execution gets its own Store). Classic CLI guests simply never link
/// against the extra host interface.
fn wasm_linker() -> Result<&'static wasmtime::component::Linker<WasmHostState>, String> {
  static LINKER: std::sync::OnceLock<Result<wasmtime::component::Linker<WasmHostState>, String>> =
    std::sync::OnceLock::new();
  LINKER
    .get_or_init(|| {
      let engine = wasm_engine()?;
      let mut linker = wasmtime::component::Linker::new(engine);
      wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
        .map_err(|err| format!("link p2 wasi: {err}"))?;
      add_task_host_to_linker(&mut linker)?;
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
/// so they run on the same p2 runtime. Adaptation happens at most once per
/// content hash — the result feeds the compile cache (review §6), so the
/// adapter cost is not paid per execution. Binary validation is a
/// debug-build check only (§6): release builds trust `Component::new`'s own
/// validation.
#[cfg(feature = "p1-compat")]
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
    .validate(cfg!(debug_assertions))
    .encode()
    .map_err(|err| format!("componentize p1 module: {err}"))
}

/// Without the `p1-compat` feature (review §8) the adapter tooling (`wat`,
/// `wit-component`, the preview1 adapter blob) is compiled out: only
/// ready-made component binaries (wasm32-wasip2 output) are accepted.
#[cfg(not(feature = "p1-compat"))]
pub fn componentize(source: &[u8]) -> Result<Vec<u8>, String> {
  if is_component_binary(source) {
    return Ok(source.to_vec());
  }
  Err(
    "this build only accepts wasm component binaries (wasm32-wasip2); WAT text and preview1 core \
     modules need the `p1-compat` feature"
      .to_string(),
  )
}

/// A compiled command component — the expensive part, cached by content
/// hash together with its (approximate) size for the byte budget.
struct CompiledWasm {
  component: wasmtime::component::Component,
  /// Component binary size — proxy for the cache entry's memory footprint.
  bytes: usize,
  /// Logical timestamp of the last cache hit (LRU eviction order).
  last_used: AtomicU64,
}

/// Pre-validation before compilation (v2 review §2): the same
/// `wasmparser` validator wasmtime uses internally, run up front so a
/// malformed or malicious binary is rejected with a precise diagnostic
/// BEFORE any cranelift compilation time is spent on it.
fn pre_validate(component_bytes: &[u8]) -> Result<(), String> {
  wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
    .validate_all(component_bytes)
    .map(|_| ())
    .map_err(|err| format!("invalid wasm component binary: {err}"))
}

impl CompiledWasm {
  /// Load a WASM source and compile it into a runnable component. It is
  /// NOT instantiated here — that happens per-execution.
  fn load(engine: &wasmtime::Engine, source: &[u8]) -> Result<Self, String> {
    let component_bytes = componentize(source)?;
    pre_validate(&component_bytes)?;
    let component = wasmtime::component::Component::new(engine, &component_bytes)
      .map_err(|err| format!("compile wasm component: {err}"))?;
    Ok(Self {
      component,
      bytes: component_bytes.len(),
      last_used: AtomicU64::new(0),
    })
  }
}

/// Cache capacity in entries and total component bytes (review §5): a
/// stream of distinct modules must not grow worker memory without bound,
/// and a handful of huge modules must not either.
const WASM_COMPILE_CACHE_CAP: usize = 64;
const WASM_COMPILE_CACHE_BYTE_CAP: usize = 256 * 1024 * 1024;

/// LRU cache of compiled components keyed by module content hash. Reads
/// (the hot path) take the read lock and bump an atomic recency stamp, so
/// concurrent executions of cached modules never contend; only a cold
/// compile takes the write lock, and compilation itself happens outside
/// any lock.
struct WasmCompileCache {
  entries: std::collections::HashMap<String, CompiledWasm>,
  total_bytes: usize,
  entry_cap: usize,
  byte_cap: usize,
  clock: AtomicU64,
}

impl WasmCompileCache {
  fn new(entry_cap: usize, byte_cap: usize) -> Self {
    Self {
      entries: std::collections::HashMap::new(),
      total_bytes: 0,
      entry_cap,
      byte_cap,
      clock: AtomicU64::new(0),
    }
  }

  /// Read-lock path: clone the Arc-backed component and stamp recency.
  fn get(&self, hash: &str) -> Option<wasmtime::component::Component> {
    let entry = self.entries.get(hash)?;
    let now = self.clock.fetch_add(1, Ordering::Relaxed) + 1;
    entry.last_used.store(now, Ordering::Relaxed);
    Some(entry.component.clone())
  }

  /// Write-lock path: insert and evict least-recently-used entries until
  /// both the entry cap and the byte budget hold. The recency order is
  /// materialized ONCE into a `BTreeSet` (stamps only move under the read
  /// lock, and we hold the write lock here), so evicting k of n entries is
  /// one O(n log n) build + k O(log n) pops instead of k full O(n) scans.
  fn insert(&mut self, hash: String, compiled: CompiledWasm) {
    if self.entries.contains_key(&hash) {
      return; // lost a compile race; keep the existing entry
    }
    let incoming = compiled.bytes;
    if !self.entries.is_empty()
      && (self.entries.len() >= self.entry_cap || self.total_bytes + incoming > self.byte_cap)
    {
      let mut by_recency: std::collections::BTreeSet<(u64, String)> = self
        .entries
        .iter()
        .map(|(key, entry)| (entry.last_used.load(Ordering::Relaxed), key.clone()))
        .collect();
      while !self.entries.is_empty()
        && (self.entries.len() >= self.entry_cap || self.total_bytes + incoming > self.byte_cap)
      {
        let Some((_, evict)) = by_recency.pop_first() else {
          break;
        };
        if let Some(evicted) = self.entries.remove(&evict) {
          self.total_bytes -= evicted.bytes;
          tracing::debug!(module_hash = %evict[.. 12.min(evict.len())], "evicted compiled wasm from cache");
        }
      }
    }
    let now = self.clock.fetch_add(1, Ordering::Relaxed) + 1;
    compiled.last_used.store(now, Ordering::Relaxed);
    self.total_bytes += incoming;
    self.entries.insert(hash, compiled);
  }
}

fn wasm_compile_cache() -> &'static std::sync::RwLock<WasmCompileCache> {
  static CACHE: std::sync::OnceLock<std::sync::RwLock<WasmCompileCache>> =
    std::sync::OnceLock::new();
  CACHE.get_or_init(|| {
    std::sync::RwLock::new(WasmCompileCache::new(
      WASM_COMPILE_CACHE_CAP,
      WASM_COMPILE_CACHE_BYTE_CAP,
    ))
  })
}

/// Per-content-hash compile locks: two tasks missing the cache on the SAME
/// module serialize on its hash lock, so the loser waits and then hits the
/// cache instead of burning a duplicate cranelift compile. Different
/// modules still compile fully in parallel.
fn compile_locks() -> &'static std::sync::Mutex<
  std::collections::HashMap<String, std::sync::Arc<std::sync::Mutex<()>>>,
> {
  static LOCKS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<std::sync::Mutex<()>>>>,
  > = std::sync::OnceLock::new();
  LOCKS.get_or_init(Default::default)
}

/// Get the compiled component for `source`, compiling and caching on miss.
/// Compilation happens OUTSIDE the cache lock so a slow compile never
/// serializes executions of other (cached) modules; concurrent misses on
/// one module are deduplicated by a per-hash lock + double-check.
fn compile_cached(source: &[u8]) -> Result<wasmtime::component::Component, String> {
  let engine = wasm_engine()?;
  let hash = module_hash(source);

  let cached = wasm_compile_cache()
    .read()
    .map_err(|_| "wasm compile cache poisoned".to_string())?
    .get(&hash);
  if let Some(component) = cached {
    tracing::debug!(module_hash = %hash[.. 12], "reusing cached compiled wasm component");
    return Ok(component);
  }

  // Serialize compiles of THIS module only. A panicked previous compile
  // poisons the hash lock; the lock protects no data, so continuing is
  // sound.
  let hash_lock = compile_locks()
    .lock()
    .expect("compile locks poisoned")
    .entry(hash.clone())
    .or_default()
    .clone();
  let _compiling = hash_lock
    .lock()
    .unwrap_or_else(|poisoned| poisoned.into_inner());

  // Double-check under the hash lock: the winner of a concurrent miss has
  // already cached the component by the time the loser gets here.
  let cached = wasm_compile_cache()
    .read()
    .map_err(|_| "wasm compile cache poisoned".to_string())?
    .get(&hash);
  if let Some(component) = cached {
    tracing::debug!(module_hash = %hash[.. 12], "wasm compile deduplicated by hash lock");
    return Ok(component);
  }

  // Structured observability (per-module compile span + duration metric):
  // cold-compile latency is the dominant first-task cost.
  let span = tracing::info_span!(
    "wasm_compile",
    module_hash = %hash[.. 12],
    source_bytes = source.len(),
  );
  let _entered = span.enter();
  let started = std::time::Instant::now();
  let compiled =
    CompiledWasm::load(engine, source).map_err(|err| format!("load wasm module: {err}"))?;
  let compile_time = started.elapsed();
  metrics::histogram!("wasm_compile_duration_seconds").record(compile_time.as_secs_f64());
  tracing::debug!(
    compile_time_ms = compile_time.as_millis() as u64,
    component_bytes = compiled.bytes,
    "compiled and cached new wasm component"
  );
  let component = compiled.component.clone();
  wasm_compile_cache()
    .write()
    .map_err(|_| "wasm compile cache poisoned".to_string())?
    .insert(hash.clone(), compiled);
  // Drop this hash's lock entry: post-insert arrivals hit the cache on the
  // fast path and never reach the lock map again.
  compile_locks()
    .lock()
    .expect("compile locks poisoned")
    .remove(&hash);
  Ok(component)
}

/// WASI ctx for one invocation: captured stdout + guest argv/env. Shared
/// between fresh Stores ([`new_store`]) and per-call re-arming in
/// [`WasmRuntime::execute_batch`].
fn build_wasi_ctx(
  invocation: &WasmInvocation,
  stdout: wasmtime_wasi::p2::pipe::MemoryOutputPipe,
) -> wasmtime_wasi::WasiCtx {
  let mut argv = vec!["task.wasm".to_string()];
  argv.extend(invocation.args.iter().cloned());
  wasmtime_wasi::WasiCtx::builder()
    .stdout(stdout)
    .args(&argv)
    .envs(&invocation.env)
    .build()
}

/// Fresh Store with WASI ctx, resource limiter, fuel budget and epoch
/// deadline — the shared sandbox setup for both execution paths.
fn new_store(
  engine: &wasmtime::Engine,
  invocation: &WasmInvocation,
  stdout: wasmtime_wasi::p2::pipe::MemoryOutputPipe,
) -> Result<wasmtime::Store<WasmHostState>, String> {
  let wasi = build_wasi_ctx(invocation, stdout);

  let wall_clock = invocation
    .wall_clock_limit
    .unwrap_or(WASM_WALL_CLOCK_BACKSTOP);
  let mut store = wasmtime::Store::new(
    engine,
    WasmHostState {
      wasi,
      table: wasmtime_wasi::ResourceTable::new(),
      limiter: ExecutionLimiter::default(),
      task_id: invocation.task_id.clone(),
      config: invocation.config.iter().cloned().collect(),
      deadline: std::time::Instant::now() + wall_clock,
    },
  );
  store.limiter(|state| &mut state.limiter);
  store
    .set_fuel(invocation.fuel_limit.unwrap_or(WASM_FUEL_LIMIT))
    .map_err(|err| format!("set wasm fuel: {err}"))?;
  store.epoch_deadline_trap();
  store.set_epoch_deadline(epoch_deadline_ticks(wall_clock));
  Ok(store)
}

/// Fuel burned so far in `store` against `fuel_limit`.
fn fuel_used(store: &wasmtime::Store<WasmHostState>, fuel_limit: u64) -> u64 {
  fuel_limit.saturating_sub(store.get_fuel().unwrap_or(0))
}

/// Turn a wasmtime error into the task-facing message, naming the two
/// sandbox trips (fuel, wall clock) explicitly so operators can tell them
/// from guest bugs.
fn describe_wasm_error(err: &wasmtime::Error, fuel_limit: u64) -> String {
  if let Some(trap) = err.downcast_ref::<wasmtime::Trap>() {
    return match trap {
      wasmtime::Trap::OutOfFuel => {
        format!("wasm ran out of fuel (budget {fuel_limit})")
      }
      wasmtime::Trap::Interrupt => {
        "wasm exceeded the wall-clock backstop (epoch deadline)".to_string()
      }
      other => format!("wasm execution trapped: {other:?}"),
    };
  }
  format!("wasm execution trapped: {err}")
}

/// A failed typed-runner call, carrying whether the failure came from the
/// ENGINE (trap, fuel/deadline exhaustion, canonical-ABI error — the
/// shared batch instance is left in an undefined state and must not be
/// reused) or from the GUEST's error string (instance stays healthy).
/// Derived from the `wasmtime::Error` structure, not from matching the
/// rendered message.
struct WasmCallFailure {
  message: String,
  poisons_instance: bool,
}

/// Does the component export the typed `cluster:task/runner` interface?
fn has_typed_runner(engine: &wasmtime::Engine, component: &wasmtime::component::Component) -> bool {
  component
    .component_type()
    .exports(engine)
    .any(|(name, _)| name == "cluster:task/runner")
}

/// Run one typed invocation against an already-instantiated runner.
fn call_typed_runner(
  bindings: &typed::TaskExecutor,
  store: &mut wasmtime::Store<WasmHostState>,
  invocation: &WasmInvocation,
) -> Result<WasmOutcome, WasmCallFailure> {
  let fuel_limit = invocation.fuel_limit.unwrap_or(WASM_FUEL_LIMIT);
  let input = typed::exports::cluster::task::runner::TaskInput {
    args: invocation.args.clone(),
    env: invocation
      .env
      .iter()
      .map(|(key, value)| format!("{key}={value}"))
      .collect(),
  };
  match bindings.cluster_task_runner().call_run(&mut *store, &input) {
    Ok(Ok(output)) => Ok(WasmOutcome {
      stdout: output.stdout,
      fuel_used: Some(fuel_used(store, fuel_limit)),
      structured: output.structured,
    }),
    // The guest-level error string is a task failure (retry path); the
    // instance completed the call cleanly and stays reusable.
    Ok(Err(guest_error)) => Err(WasmCallFailure {
      message: format!("wasm task returned error: {guest_error}"),
      poisons_instance: false,
    }),
    // Any engine-level error (trap, fuel, epoch deadline, ABI failure)
    // leaves the instance state undefined.
    Err(err) => Err(WasmCallFailure {
      message: describe_wasm_error(&err, fuel_limit),
      poisons_instance: true,
    }),
  }
}

/// Run one `wasi:cli/run` command invocation (classic guests): stdout is
/// the result, exit codes and traps are failures.
fn run_cli_command(
  component: &wasmtime::component::Component,
  invocation: &WasmInvocation,
) -> Result<WasmOutcome, String> {
  let engine = wasm_engine()?;
  let fuel_limit = invocation.fuel_limit.unwrap_or(WASM_FUEL_LIMIT);
  let stdout = wasmtime_wasi::p2::pipe::MemoryOutputPipe::new(WASM_STDOUT_CAPACITY);
  let mut store = new_store(engine, invocation, stdout.clone())?;

  let command =
    wasmtime_wasi::p2::bindings::sync::Command::instantiate(&mut store, component, wasm_linker()?)
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
        None => return Err(describe_wasm_error(&err, fuel_limit)),
      }
    }
  }

  let fuel = fuel_used(&store, fuel_limit);
  let output = String::from_utf8_lossy(&stdout.contents()).into_owned();
  Ok(WasmOutcome {
    stdout: output,
    fuel_used: Some(fuel),
    structured: None,
  })
}

impl WasmRuntime for WasmtimeRuntime {
  fn name(&self) -> &'static str {
    "wasmtime"
  }

  fn execute(&self, source: &[u8], invocation: WasmInvocation) -> Result<WasmOutcome, String> {
    let engine = wasm_engine()?;
    let component = compile_cached(source)?;

    if has_typed_runner(engine, &component) {
      let stdout = wasmtime_wasi::p2::pipe::MemoryOutputPipe::new(WASM_STDOUT_CAPACITY);
      let mut store = new_store(engine, &invocation, stdout)?;
      let bindings = typed::TaskExecutor::instantiate(&mut store, &component, wasm_linker()?)
        .map_err(|err| format!("instantiate typed wasm component: {err}"))?;
      return call_typed_runner(&bindings, &mut store, &invocation)
        .map_err(|failure| failure.message);
    }

    run_cli_command(&component, &invocation)
  }

  /// Batch execution (review §4): typed runners are instantiated ONCE and
  /// called per invocation — one Store, one instance, N calls — with the
  /// fuel budget and epoch deadline re-armed between calls. A trap poisons
  /// the shared instance, so remaining invocations fail fast with the trap
  /// error. CLI command components are one-shot by design (`_start` →
  /// exit), so they fall back to the per-invocation loop.
  fn execute_batch(
    &self,
    source: &[u8],
    invocations: Vec<WasmInvocation>,
  ) -> Vec<Result<WasmOutcome, String>> {
    let setup = (|| -> Result<_, String> {
      let engine = wasm_engine()?;
      let component = compile_cached(source)?;
      Ok((engine, component))
    })();
    let (engine, component) = match setup {
      Ok(pair) => pair,
      Err(err) => return invocations.iter().map(|_| Err(err.clone())).collect(),
    };

    if !has_typed_runner(engine, &component) {
      return invocations
        .into_iter()
        .map(|invocation| run_cli_command(&component, &invocation))
        .collect();
    }

    let batch_setup = (|| -> Result<_, String> {
      let stdout = wasmtime_wasi::p2::pipe::MemoryOutputPipe::new(WASM_STDOUT_CAPACITY);
      let first = invocations.first().cloned().unwrap_or_default();
      let mut store = new_store(engine, &first, stdout)?;
      let bindings = typed::TaskExecutor::instantiate(&mut store, &component, wasm_linker()?)
        .map_err(|err| format!("instantiate typed wasm component: {err}"))?;
      Ok((store, bindings))
    })();
    let (mut store, bindings) = match batch_setup {
      Ok(pair) => pair,
      Err(err) => return invocations.iter().map(|_| Err(err.clone())).collect(),
    };

    let mut results = Vec::with_capacity(invocations.len());
    let mut poisoned: Option<String> = None;
    for invocation in &invocations {
      if let Some(trap) = &poisoned {
        results.push(Err(format!("skipped after earlier trap in batch: {trap}")));
        continue;
      }
      // Re-arm the sandbox budgets for this call on the shared Store.
      if let Err(err) = store.set_fuel(invocation.fuel_limit.unwrap_or(WASM_FUEL_LIMIT)) {
        results.push(Err(format!("set wasm fuel: {err}")));
        continue;
      }
      let wall_clock = invocation
        .wall_clock_limit
        .unwrap_or(WASM_WALL_CLOCK_BACKSTOP);
      store.set_epoch_deadline(epoch_deadline_ticks(wall_clock));
      // Re-arm the host-call state per invocation too (v2 review §10),
      // INCLUDING the WASI ctx: typed runners receive args/env through
      // the task-input record, but a guest peeking at WASI argv/env must
      // see THIS call's values, not the first invocation's. The
      // ResourceTable is kept — dropping it would invalidate handles the
      // live instance may still hold.
      let state = store.data_mut();
      state.task_id = invocation.task_id.clone();
      state.config = invocation.config.iter().cloned().collect();
      state.deadline = std::time::Instant::now() + wall_clock;
      state.wasi = build_wasi_ctx(
        invocation,
        wasmtime_wasi::p2::pipe::MemoryOutputPipe::new(WASM_STDOUT_CAPACITY),
      );
      let result = call_typed_runner(&bindings, &mut store, invocation);
      if let Err(failure) = &result {
        // An engine-level failure (trap, fuel, deadline) leaves the
        // shared instance unusable for the rest of the batch; a
        // guest-level error string does not. The flag comes from the
        // error's structure (wasmtime::Trap downcast), not from string
        // matching on the message.
        if failure.poisons_instance {
          poisoned = Some(failure.message.clone());
        }
      }
      results.push(result.map_err(|failure| failure.message));
    }
    results
  }

  fn prepare(&self, source: &[u8]) -> Result<(), String> {
    compile_cached(source).map(|_| ())
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
    assert_eq!(store.list(), vec!["bin.wasm", "hello.wat"]);
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
  #[cfg(feature = "p1-compat")]
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
    assert!(outcome.structured.is_none());
  }

  /// Review §2: memory growth past the limiter cap is denied — the guest
  /// sees `memory.grow` return -1 while growth under the cap succeeds.
  #[cfg(feature = "p1-compat")]
  #[test]
  fn memory_limiter_denies_growth_past_cap() {
    let runtime: &dyn WasmRuntime = runtime_by_name("wasmtime").unwrap();
    // Grows by 8 pages (fine), then by 2000 pages (128MB > 64MB cap, must
    // be denied → -1 → guest exits cleanly; a non--1 result traps).
    let wat = r#"
      (module
        (memory 1)
        (export "memory" (memory 0))
        (func (export "_start")
          (if (i32.eq (memory.grow (i32.const 8)) (i32.const -1))
            (then unreachable))
          (if (i32.ne (memory.grow (i32.const 2000)) (i32.const -1))
            (then unreachable))))
    "#;
    let outcome = runtime.execute(wat.as_bytes(), WasmInvocation::default());
    assert!(
      outcome.is_ok(),
      "limiter should deny the huge grow without trapping: {outcome:?}"
    );
  }

  /// Review §2: the epoch deadline interrupts a spin loop long before its
  /// (huge) fuel budget runs out — the wall-clock backstop works.
  #[cfg(feature = "p1-compat")]
  #[test]
  fn epoch_deadline_interrupts_spin_loop() {
    let runtime: &dyn WasmRuntime = runtime_by_name("wasmtime").unwrap();
    let spin_wat = r#"
      (module
        (memory 1)
        (export "memory" (memory 0))
        (func (export "_start") (loop br 0)))
    "#;
    let started = std::time::Instant::now();
    let err = runtime
      .execute(
        spin_wat.as_bytes(),
        WasmInvocation {
          fuel_limit: Some(u64::MAX / 2),
          wall_clock_limit: Some(Duration::from_millis(300)),
          ..Default::default()
        },
      )
      .unwrap_err();
    assert!(
      err.contains("wall-clock"),
      "expected epoch interrupt, got: {err}"
    );
    assert!(
      started.elapsed() < Duration::from_secs(10),
      "interrupt took too long: {:?}",
      started.elapsed()
    );
  }

  /// Review §5: LRU + byte-budget eviction. A small cache keeps the
  /// recently-used entry and evicts the stale one.
  #[cfg(feature = "p1-compat")]
  #[test]
  fn compile_cache_evicts_least_recently_used() {
    let engine = wasm_engine().unwrap();
    let compile = |tag: &str| {
      let wat = format!(
        r#"(module (memory 1) (export "memory" (memory 0)) (data (i32.const 8) "{tag}") (func (export "_start")))"#
      );
      CompiledWasm::load(engine, wat.as_bytes()).unwrap()
    };
    let mut cache = WasmCompileCache::new(2, usize::MAX);
    cache.insert("a".into(), compile("a"));
    cache.insert("b".into(), compile("b"));
    assert!(cache.get("a").is_some()); // bump a's recency; b is now LRU
    cache.insert("c".into(), compile("c"));
    assert!(cache.get("a").is_some(), "recently used entry evicted");
    assert!(cache.get("b").is_none(), "LRU entry should be evicted");
    assert!(cache.get("c").is_some());

    // Byte budget: a cache with a tiny byte cap holds at most one entry.
    let mut tiny = WasmCompileCache::new(64, 1);
    tiny.insert("a".into(), compile("a"));
    tiny.insert("b".into(), compile("b"));
    assert!(tiny.get("a").is_none(), "byte budget should evict a");
    assert!(tiny.get("b").is_some());
  }

  /// A hand-written component implementing `cluster:task/runner` (the
  /// typed path of review §3/§7): returns a typed record instead of
  /// writing stdout, and calls the `cluster:task/host.log` import.
  #[cfg(feature = "p1-compat")]
  const TYPED_RUNNER_WAT: &str = r#"
    (component
      (import "cluster:task/host" (instance $host
        (export "log" (func (param "message" string)))))

      (core module $memory_mod
        (memory (export "memory") 2))
      (core instance $mem_inst (instantiate $memory_mod))
      (alias core export $mem_inst "memory" (core memory $guest_mem))

      (core func $log_lowered (canon lower (func $host "log")
        (memory $guest_mem) string-encoding=utf8))

      (core module $impl
        (import "env" "memory" (memory 2))
        (import "host" "log" (func $log (param i32 i32)))
        (global $bump (mut i32) (i32.const 65536))
        (data (i32.const 16) "typed-ok\n")
        (data (i32.const 32) "{\22typed\22:true}")
        (data (i32.const 56) "typed guest running")
        (func (export "cabi_realloc")
          (param $old i32) (param $oldsz i32) (param $align i32) (param $size i32) (result i32)
          (local $ptr i32)
          (local.set $ptr
            (i32.and
              (i32.add (global.get $bump) (i32.sub (local.get $align) (i32.const 1)))
              (i32.xor (i32.sub (local.get $align) (i32.const 1)) (i32.const -1))))
          (global.set $bump (i32.add (local.get $ptr) (local.get $size)))
          (local.get $ptr))
        (func (export "run") (param i32 i32 i32 i32) (result i32)
          (call $log (i32.const 56) (i32.const 19))
          ;; result<task-output, string>: ok discriminant + task-output
          (i32.store8 (i32.const 128) (i32.const 0))   ;; ok
          (i32.store (i32.const 132) (i32.const 16))   ;; stdout.ptr
          (i32.store (i32.const 136) (i32.const 9))    ;; stdout.len
          (i32.store8 (i32.const 140) (i32.const 1))   ;; structured = some
          (i32.store (i32.const 144) (i32.const 32))   ;; structured.ptr
          (i32.store (i32.const 148) (i32.const 14))   ;; structured.len
          (i32.const 128)))

      (core instance $impl_inst (instantiate $impl
        (with "env" (instance (export "memory" (memory $guest_mem))))
        (with "host" (instance (export "log" (func $log_lowered))))))

      (alias core export $impl_inst "run" (core func $run_core))
      (alias core export $impl_inst "cabi_realloc" (core func $realloc_core))

      (type $task-input (record
        (field "args" (list string))
        (field "env" (list string))))
      (type $task-output (record
        (field "stdout" string)
        (field "structured" (option string))))

      (func $run (param "input" $task-input) (result (result $task-output (error string)))
        (canon lift (core func $run_core)
          (memory $guest_mem) (realloc (func $realloc_core)) string-encoding=utf8))

      (instance $runner
        (export "task-input" (type $task-input))
        (export "task-output" (type $task-output))
        (export "run" (func $run)))

      (export "cluster:task/runner" (instance $runner))
    )
  "#;

  #[cfg(feature = "p1-compat")]
  #[test]
  fn typed_runner_component_returns_records() {
    let runtime: &dyn WasmRuntime = runtime_by_name("wasmtime").unwrap();
    let component = wat::parse_str(TYPED_RUNNER_WAT).expect("assemble typed component");
    let outcome = runtime
      .execute(
        &component,
        WasmInvocation {
          args: vec!["a".to_string(), "b".to_string()],
          env: vec![("K".to_string(), "V".to_string())],
          ..Default::default()
        },
      )
      .unwrap();
    assert_eq!(outcome.stdout, "typed-ok\n");
    assert_eq!(outcome.structured.as_deref(), Some(r#"{"typed":true}"#));
    assert!(outcome.fuel_used.unwrap() > 0);
  }

  /// Review §4: a typed batch reuses one instance for N calls; every call
  /// gets its own outcome and its own re-armed fuel budget.
  #[cfg(feature = "p1-compat")]
  #[test]
  fn typed_batch_executes_all_invocations() {
    let runtime: &dyn WasmRuntime = runtime_by_name("wasmtime").unwrap();
    let component = wat::parse_str(TYPED_RUNNER_WAT).expect("assemble typed component");
    let invocations = vec![WasmInvocation::default(), WasmInvocation::default()];
    let results = runtime.execute_batch(&component, invocations);
    assert_eq!(results.len(), 2);
    for result in results {
      let outcome = result.unwrap();
      assert_eq!(outcome.stdout, "typed-ok\n");
      assert_eq!(outcome.structured.as_deref(), Some(r#"{"typed":true}"#));
    }
  }

  /// The batch default also covers classic CLI guests (loop fallback).
  #[cfg(feature = "p1-compat")]
  #[test]
  fn cli_batch_falls_back_to_per_invocation_loop() {
    let runtime: &dyn WasmRuntime = runtime_by_name("wasmtime").unwrap();
    let wat = r#"
      (module
        (memory 1)
        (export "memory" (memory 0))
        (func (export "_start")))
    "#;
    let results = runtime.execute_batch(wat.as_bytes(), vec![WasmInvocation::default(); 3]);
    assert_eq!(results.len(), 3);
    assert!(results.into_iter().all(|r| r.is_ok()));
  }

  /// Bytes of the real wit-bindgen guest (auto-built by build.rs, v2
  /// review §7); `None` when the wasm32-wasip2 target is unavailable.
  fn typed_hello_guest() -> Option<Vec<u8>> {
    let path = concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/wasm_guests/typed_hello/target/wasm32-wasip2/release/typed_hello.wasm"
    );
    std::fs::read(path).ok()
  }

  /// End-to-end typed path with a REAL wit-bindgen guest, exercising the
  /// extended host surface (v2 review §4/§5): task-id, fuel-remaining,
  /// wall-clock-remaining-ms, report-progress and config-get.
  #[test]
  fn rust_typed_guest_runs_when_built() {
    let Some(bytes) = typed_hello_guest() else {
      eprintln!("typed_hello guest not built; skipping");
      return;
    };
    let runtime: &dyn WasmRuntime = runtime_by_name("wasmtime").unwrap();
    let outcome = runtime
      .execute(
        &bytes,
        WasmInvocation {
          args: vec!["41".to_string()],
          task_id: Some("test-task".to_string()),
          ..Default::default()
        },
      )
      .unwrap();
    assert!(
      outcome.stdout.contains("41 + 1 = 42"),
      "unexpected stdout: {}",
      outcome.stdout
    );
    assert_eq!(
      outcome.structured.as_deref(),
      Some(r#"{"input":41,"answer":42,"generation":1}"#)
    );

    // config-get feeds guest logic: bonus=2 shifts the answer.
    let outcome = runtime
      .execute(
        &bytes,
        WasmInvocation {
          args: vec!["41".to_string()],
          config: vec![("bonus".to_string(), "2".to_string())],
          ..Default::default()
        },
      )
      .unwrap();
    assert_eq!(
      outcome.structured.as_deref(),
      Some(r#"{"input":41,"answer":44,"generation":1}"#)
    );
  }

  /// v2 review §2: a malformed binary is rejected by wasmparser
  /// pre-validation with a precise diagnostic, before compilation.
  #[test]
  fn pre_validation_rejects_malformed_binary() {
    let err = pre_validate(&[0x00, 0x61, 0x73, 0x6d, 0xff, 0xff, 0xff, 0xff]).unwrap_err();
    assert!(err.contains("invalid wasm"), "unexpected error: {err}");
    // A well-formed component passes.
    #[cfg(feature = "p1-compat")]
    {
      let component = wat::parse_str(TYPED_RUNNER_WAT).expect("assemble typed component");
      pre_validate(&component).expect("valid component must pass pre-validation");
    }
  }

  /// v2 review §11 mock strategy (pattern from
  /// `wasmtime_actor/tests/wasm_actor_mockall.rs`): the guest's
  /// host-visible flow is mirrored natively and driven against a mocked
  /// [`TaskHostOps`], verifying host-call sequencing and config plumbing
  /// without instantiating any wasm.
  #[test]
  fn mocked_host_sees_guest_call_sequence() {
    use mockall::predicate::eq;

    /// Native mirror of typed_hello's host interaction.
    fn guest_mirror(host: &mut dyn TaskHostOps, n: u64) -> u64 {
      let task_id = host.task_id();
      host.log(&format!("start task {task_id}"));
      let bonus: u64 = host
        .config_get("bonus")
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(0);
      host.report_progress(1, 1);
      n + 1 + bonus
    }

    let mut host = MockTaskHostOps::new();
    host
      .expect_task_id()
      .times(1)
      .return_const("task-9".to_string());
    host
      .expect_log()
      .withf(|message| message.contains("task-9"))
      .times(1)
      .return_const(());
    host
      .expect_config_get()
      .with(eq("bonus"))
      .times(1)
      .return_const(Some("2".to_string()));
    host
      .expect_report_progress()
      .with(eq(1), eq(1))
      .times(1)
      .return_const(());

    assert_eq!(guest_mirror(&mut host, 41), 44);
  }

  /// `host.kv-get` routes through the process-wide registered reader and
  /// answers none for unknown group/key pairs (and before registration on
  /// client binaries).
  #[test]
  fn kv_get_host_call_uses_registered_reader() {
    register_wasm_kv_reader(std::sync::Arc::new(|group, key| {
      (group == "users" && key == "answer").then(|| "42".to_string())
    }));
    let state = WasmHostState {
      wasi: build_wasi_ctx(
        &WasmInvocation::default(),
        wasmtime_wasi::p2::pipe::MemoryOutputPipe::new(WASM_STDOUT_CAPACITY),
      ),
      table: wasmtime_wasi::ResourceTable::new(),
      limiter: ExecutionLimiter::default(),
      task_id: None,
      config: Default::default(),
      deadline: std::time::Instant::now() + WASM_WALL_CLOCK_BACKSTOP,
    };
    assert_eq!(state.kv_get("users", "answer").as_deref(), Some("42"));
    assert_eq!(state.kv_get("users", "missing"), None);
    assert_eq!(state.kv_get("orders", "answer"), None);
  }

  proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig {
      cases: 8, // each case runs real wasm twice; keep the budget small
      ..Default::default()
    })]

    /// v2 review §11 property strategy: the typed guest is DETERMINISTIC
    /// (same input → identical records twice) and matches the native
    /// model (answer = n + 1 + bonus) across random inputs.
    #[test]
    fn typed_guest_is_deterministic_and_matches_model(
      n in 0u64 .. 1_000_000,
      bonus in 0u64 .. 1_000,
    ) {
      let Some(bytes) = typed_hello_guest() else {
        return Ok(()); // guest not built; property vacuously holds
      };
      let runtime: &dyn WasmRuntime = runtime_by_name("wasmtime").unwrap();
      let invocation = WasmInvocation {
        args: vec![n.to_string()],
        config: vec![("bonus".to_string(), bonus.to_string())],
        ..Default::default()
      };
      let first = runtime.execute(&bytes, invocation.clone()).unwrap();
      let second = runtime.execute(&bytes, invocation).unwrap();
      let expected = format!(
        r#"{{"input":{n},"answer":{},"generation":1}}"#,
        n + 1 + bonus
      );
      proptest::prop_assert_eq!(first.stdout.clone(), second.stdout);
      proptest::prop_assert_eq!(first.structured.as_deref(), Some(expected.as_str()));
      proptest::prop_assert_eq!(first.structured, second.structured);
    }
  }

  /// The pooled entry point used by the task handler (review §1).
  #[cfg(feature = "p1-compat")]
  #[tokio::test]
  async fn execute_pooled_runs_on_dedicated_pool() {
    let runtime = runtime_by_name("wasmtime").unwrap();
    let wat = r#"
      (module
        (memory 1)
        (export "memory" (memory 0))
        (func (export "_start")))
    "#;
    let outcome = execute_pooled(runtime, wat.as_bytes().to_vec(), WasmInvocation::default())
      .await
      .unwrap();
    assert_eq!(outcome.stdout, "");
  }
}

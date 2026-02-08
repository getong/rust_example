// Copyright 2018-2026 the Deno authors. MIT license.

use std::{
  path::PathBuf,
  sync::{Arc, Mutex},
  task::{Context, Poll},
  thread,
  time::Duration,
};

use base64::Engine;
use deno_core::{
  JsRuntime, ModuleSpecifier, PollEventLoopOptions,
  error::AnyError,
  futures::task::noop_waker_ref,
  scope, serde_v8,
  v8::{self, Local},
};
use deno_lib::{
  npm::create_npm_process_state_provider,
  worker::{LibMainWorkerFactory, LibWorkerFactoryRoots},
};
use deno_path_util::resolve_url_or_path;
use deno_runtime::{WorkerExecutionMode, worker::MainWorker};
use serde_json::Value as JsonValue;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::{
  args::{DenoSubcommand, Flags, RunFlags},
  embed::{EmbedResult, extension as embed_extension},
  factory::CliFactory,
};

pub type ModuleId = u32;

#[derive(Debug, Clone)]
pub struct LoadModuleResult {
  pub module_id: ModuleId,
  pub embed_result: Option<JsonValue>,
  pub embed_exit_data: Option<JsonValue>,
}

#[derive(Debug, Clone)]
pub struct CallModuleResult {
  pub module_id: ModuleId,
  pub result: JsonValue,
  pub embed_result: Option<JsonValue>,
  pub embed_exit_data: Option<JsonValue>,
}

#[derive(Debug)]
enum RequestKind {
  LoadModule {
    specifier: String,
  },
  CallModule {
    specifier: String,
    function: String,
    args: JsonValue,
  },
  CallModuleId {
    module_id: ModuleId,
    function: String,
    args: JsonValue,
  },
  Shutdown,
}

#[derive(Debug)]
struct Request {
  id: String,
  kind: RequestKind,
  response_tx: oneshot::Sender<Response>,
}

#[derive(Debug)]
struct Response {
  id: String,
  payload: Result<JsonValue, String>,
}

#[derive(Clone)]
pub struct DenoRuntimeHandle {
  tx: mpsc::UnboundedSender<Request>,
}

impl DenoRuntimeHandle {
  pub fn load_module(&self, specifier: impl Into<String>) -> Result<LoadModuleResult, AnyError> {
    let specifier = specifier.into();
    let id = Uuid::new_v4().to_string();
    let (response_tx, response_rx) = oneshot::channel();
    self
      .tx
      .send(Request {
        id: id.clone(),
        kind: RequestKind::LoadModule { specifier },
        response_tx,
      })
      .map_err(|e| AnyError::msg(format!("runtime channel closed: {e}")))?;

    let response = response_rx
      .blocking_recv()
      .map_err(|e| AnyError::msg(format!("runtime response dropped: {e}")))?;
    if response.id != id {
      return Err(AnyError::msg(format!(
        "Response ID mismatch: expected {id}, got {}",
        response.id
      )));
    }
    let payload = response.payload.map_err(AnyError::msg)?;
    parse_load_module_result(payload)
  }

  /// Call an exported function from a module.
  ///
  /// This takes three logical parameters: `(module, function, args)` and returns
  /// `(module_id, result)`. The module is cached and re-used by `module_id`.
  pub fn call_module_function(
    &self,
    specifier: impl Into<String>,
    function: impl Into<String>,
    args: JsonValue,
  ) -> Result<CallModuleResult, AnyError> {
    let id = Uuid::new_v4().to_string();
    let (response_tx, response_rx) = oneshot::channel();
    self
      .tx
      .send(Request {
        id: id.clone(),
        kind: RequestKind::CallModule {
          specifier: specifier.into(),
          function: function.into(),
          args,
        },
        response_tx,
      })
      .map_err(|e| AnyError::msg(format!("runtime channel closed: {e}")))?;

    let response = response_rx
      .blocking_recv()
      .map_err(|e| AnyError::msg(format!("runtime response dropped: {e}")))?;
    if response.id != id {
      return Err(AnyError::msg(format!(
        "Response ID mismatch: expected {id}, got {}",
        response.id
      )));
    }
    let payload = response.payload.map_err(AnyError::msg)?;
    parse_call_module_result(payload)
  }

  pub fn call_module_function_by_id(
    &self,
    module_id: ModuleId,
    function: impl Into<String>,
    args: JsonValue,
  ) -> Result<CallModuleResult, AnyError> {
    let id = Uuid::new_v4().to_string();
    let (response_tx, response_rx) = oneshot::channel();
    self
      .tx
      .send(Request {
        id: id.clone(),
        kind: RequestKind::CallModuleId {
          module_id,
          function: function.into(),
          args,
        },
        response_tx,
      })
      .map_err(|e| AnyError::msg(format!("runtime channel closed: {e}")))?;

    let response = response_rx
      .blocking_recv()
      .map_err(|e| AnyError::msg(format!("runtime response dropped: {e}")))?;
    if response.id != id {
      return Err(AnyError::msg(format!(
        "Response ID mismatch: expected {id}, got {}",
        response.id
      )));
    }
    let payload = response.payload.map_err(AnyError::msg)?;
    parse_call_module_result(payload)
  }

  pub fn shutdown(&self) {
    let id = Uuid::new_v4().to_string();
    let (response_tx, _response_rx) = oneshot::channel();
    // Ignore errors on shutdown.
    let _ = self.tx.send(Request {
      id,
      kind: RequestKind::Shutdown,
      response_tx,
    });
  }
}

pub struct DenoRuntimeOptions {
  pub initial_cwd: Option<PathBuf>,
  pub argv: Vec<String>,
  pub roots: LibWorkerFactoryRoots,
  pub embed_result: Arc<Mutex<EmbedResult>>,
}

impl Default for DenoRuntimeOptions {
  fn default() -> Self {
    Self {
      initial_cwd: None,
      argv: Vec::new(),
      roots: LibWorkerFactoryRoots::default(),
      embed_result: Arc::new(Mutex::new(EmbedResult::default())),
    }
  }
}

/// Spawns a long-lived Deno runtime on a dedicated thread and returns a handle
/// that communicates via an in-process mpsc channel.
pub fn spawn_runtime(options: DenoRuntimeOptions) -> Result<DenoRuntimeHandle, AnyError> {
  // Required by rustls 0.23+ (used throughout the vendored Deno CLI stack).
  let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

  if std::env::var_os("DENO_DIR").is_none() {
    let deno_dir = std::env::temp_dir().join("embed_deno_deno_dir");
    std::fs::create_dir_all(&deno_dir)?;
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
      std::env::set_var("DENO_DIR", &deno_dir);
    }
  }

  let (tx_outside, rx_inside) = mpsc::unbounded_channel::<Request>();

  // Startup handshake so we can surface initialization errors.
  let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();

  thread::Builder::new()
    .name("embed_deno_runtime".to_string())
    .spawn(move || {
      let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
      let rt = match rt {
        Ok(rt) => rt,
        Err(err) => {
          let _ = ready_tx.send(Err(err.to_string()));
          return;
        }
      };

      rt.block_on(async move {
        match DenoRuntimeManager::new(options).await {
          Ok(mut manager) => {
            let _ = ready_tx.send(Ok(()));
            manager.run(rx_inside).await;
          }
          Err(err) => {
            let _ = ready_tx.send(Err(err.to_string()));
          }
        }
      });
    })
    .map_err(|e| AnyError::msg(format!("failed to spawn runtime thread: {e}")))?;

  match ready_rx.recv() {
    Ok(Ok(())) => Ok(DenoRuntimeHandle { tx: tx_outside }),
    Ok(Err(err)) => Err(AnyError::msg(err)),
    Err(err) => Err(AnyError::msg(format!(
      "runtime startup channel error: {err}"
    ))),
  }
}

struct DenoRuntimeManager {
  worker: MainWorker,
  embed_result: Arc<Mutex<EmbedResult>>,
  initial_cwd: PathBuf,
}

impl DenoRuntimeManager {
  async fn new(options: DenoRuntimeOptions) -> Result<Self, AnyError> {
    let DenoRuntimeOptions {
      initial_cwd: maybe_initial_cwd,
      argv,
      roots,
      embed_result,
    } = options;

    let initial_cwd = maybe_initial_cwd
      .clone()
      .or_else(|| std::env::current_dir().ok())
      .unwrap_or_else(|| PathBuf::from("/"));

    let bootstrap_module = bootstrap_specifier()?;

    let mut flags = Flags::default();
    flags.initial_cwd = Some(initial_cwd.clone());
    flags.argv = argv;
    flags.permissions.allow_all = true;
    flags.subcommand = DenoSubcommand::Run(RunFlags {
      script: bootstrap_module.to_string(),
      ..Default::default()
    });

    let flags = Arc::new(flags);
    let factory = CliFactory::from_flags(flags.clone());
    let cli_options = factory.cli_options()?;

    let main_module = cli_options.resolve_main_module()?;
    let preload_modules = cli_options.preload_modules()?;
    let require_modules = cli_options.require_modules()?;

    let module_loader_factory = factory.create_module_loader_factory().await?;
    factory.maybe_start_inspector_server()?;
    let node_resolver = factory.node_resolver().await?.clone();
    let npm_resolver = factory.npm_resolver().await?.clone();
    let pkg_json_resolver = factory.pkg_json_resolver()?.clone();
    let fs = factory.fs().clone();

    let lib_main_worker_factory = LibMainWorkerFactory::new(
      factory.blob_store().clone(),
      if cli_options.code_cache_enabled() {
        Some(factory.code_cache()?.clone())
      } else {
        None
      },
      None, // DenoRtNativeAddonLoader
      factory.feature_checker()?.clone(),
      fs,
      cli_options.coverage_dir(),
      Box::new(module_loader_factory),
      node_resolver,
      create_npm_process_state_provider(&npm_resolver),
      pkg_json_resolver,
      factory.root_cert_store_provider().clone(),
      cli_options.resolve_storage_key_resolver(),
      factory.sys(),
      factory.create_lib_main_worker_options()?,
      roots,
      None,
    );

    let mut cli_worker = lib_main_worker_factory
      .create_custom_worker(
        WorkerExecutionMode::Run,
        main_module.clone(),
        preload_modules,
        require_modules,
        factory.root_permissions_container()?.clone(),
        vec![embed_extension(embed_result.clone())],
        Default::default(),
        None,
      )?
      .into_main_worker();

    // Execute the bootstrap module.
    cli_worker.execute_main_module(&main_module).await?;
    cli_worker.dispatch_load_event()?;
    // Don't run the event loop to completion here: long-lived modules may keep
    // pending ops (timers, watchers, connections). We'll keep pumping the event
    // loop in the daemon loop via `tokio::select!`.
    pump_event_loop_once(&mut cli_worker)?;

    Ok(Self {
      worker: cli_worker,
      embed_result,
      initial_cwd,
    })
  }

  async fn run(&mut self, mut rx: mpsc::UnboundedReceiver<Request>) {
    let mut tick = tokio::time::interval(Duration::from_millis(20));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    tick.tick().await;

    loop {
      tokio::select! {
        maybe_request = rx.recv() => {
          let Some(request) = maybe_request else {
            break;
          };

          let response = self.handle_request(request.id.clone(), request.kind).await;
          let is_shutdown =
            matches!(&response.payload, Ok(JsonValue::String(s)) if s == "__shutdown__");
          let _ = request.response_tx.send(response);
          if is_shutdown {
            break;
          }
        }

        _ = tick.tick() => {
          if let Err(err) = pump_event_loop_once(&mut self.worker) {
            eprintln!("runtime event loop error: {err:#}");
          }
        }
      }
    }
  }

  async fn handle_request(&mut self, id: String, kind: RequestKind) -> Response {
    let payload = match kind {
      RequestKind::LoadModule { specifier } => self.load_module_impl(specifier).await,
      RequestKind::CallModule {
        specifier,
        function,
        args,
      } => self.call_module_impl(specifier, function, args).await,
      RequestKind::CallModuleId {
        module_id,
        function,
        args,
      } => self.call_module_id_impl(module_id, function, args).await,
      RequestKind::Shutdown => Ok(JsonValue::String("__shutdown__".to_string())),
    }
    .map_err(|e| e.to_string());

    Response { id, payload }
  }

  fn take_embed_payload(&self) -> (Option<JsonValue>, Option<JsonValue>) {
    let Ok(mut guard) = self.embed_result.lock() else {
      return (None, None);
    };
    let result = guard
      .result
      .take()
      .and_then(|s| serde_json::from_str::<JsonValue>(&s).ok());
    let exit_data = guard
      .exit_data
      .take()
      .and_then(|s| serde_json::from_str::<JsonValue>(&s).ok());
    (result, exit_data)
  }

  async fn load_module_impl(&mut self, specifier: String) -> Result<JsonValue, AnyError> {
    let resolved = resolve_url_or_path(&specifier, &self.initial_cwd)?;
    let resolved = resolved.to_string();

    let script = format!(
      r#"(async () => {{
  globalThis.__embedDeno = globalThis.__embedDeno ?? {{
    nextModuleId: 1,
    modules: new Map(),
    specToId: new Map(),
  }};
  const spec = {spec:?};
  const existing = globalThis.__embedDeno.specToId.get(spec);
  if (existing) return {{ moduleId: existing }};
  const module = await import(spec);
  const id = globalThis.__embedDeno.nextModuleId++;
  globalThis.__embedDeno.modules.set(id, module);
  globalThis.__embedDeno.specToId.set(spec, id);
  return {{ moduleId: id }};
}})();"#,
      spec = resolved,
    );

    let json = eval_script(&mut self.worker, "[load_module]", script).await?;
    let (embed_result, embed_exit_data) = self.take_embed_payload();
    let module_id = json
      .get("moduleId")
      .and_then(|v| v.as_u64())
      .ok_or_else(|| AnyError::msg("load_module did not return moduleId"))?
      as ModuleId;

    Ok(JsonValue::Object(
      [
        ("module_id".to_string(), JsonValue::from(module_id)),
        (
          "embed_result".to_string(),
          embed_result.unwrap_or(JsonValue::Null),
        ),
        (
          "embed_exit_data".to_string(),
          embed_exit_data.unwrap_or(JsonValue::Null),
        ),
      ]
      .into_iter()
      .collect(),
    ))
  }

  async fn call_module_impl(
    &mut self,
    specifier: String,
    function: String,
    args: JsonValue,
  ) -> Result<JsonValue, AnyError> {
    if !args.is_array() {
      return Err(AnyError::msg("args must be a JSON array"));
    }
    let resolved = resolve_url_or_path(&specifier, &self.initial_cwd)?;
    let resolved = resolved.to_string();
    let args_json = serde_json::to_string(&args)?;

    let script = format!(
      r#"(async () => {{
  globalThis.__embedDeno = globalThis.__embedDeno ?? {{
    nextModuleId: 1,
    modules: new Map(),
    specToId: new Map(),
  }};
  const spec = {spec:?};
  let id = globalThis.__embedDeno.specToId.get(spec);
  if (!id) {{
    const module = await import(spec);
    id = globalThis.__embedDeno.nextModuleId++;
    globalThis.__embedDeno.modules.set(id, module);
    globalThis.__embedDeno.specToId.set(spec, id);
  }}
  const module = globalThis.__embedDeno.modules.get(id);
  if (!module) throw new Error(`Module not found in registry (id=${{id}})`);
  const funcName = {func:?};
  const fn = module[funcName];
  if (typeof fn !== 'function') throw new Error(`Export '${{funcName}}' is not a function`);
  const args = {args};
  const result = await fn(...args);
  return {{ moduleId: id, result: result ?? null }};
}})();"#,
      spec = resolved,
      func = function,
      args = args_json,
    );

    let json = eval_script(&mut self.worker, "[call_module]", script).await?;
    let (embed_result, embed_exit_data) = self.take_embed_payload();
    let module_id = json
      .get("moduleId")
      .and_then(|v| v.as_u64())
      .ok_or_else(|| AnyError::msg("call did not return moduleId"))?
      as ModuleId;
    let result = json.get("result").cloned().unwrap_or(JsonValue::Null);

    Ok(JsonValue::Object(
      [
        ("module_id".to_string(), JsonValue::from(module_id)),
        ("result".to_string(), result),
        (
          "embed_result".to_string(),
          embed_result.unwrap_or(JsonValue::Null),
        ),
        (
          "embed_exit_data".to_string(),
          embed_exit_data.unwrap_or(JsonValue::Null),
        ),
      ]
      .into_iter()
      .collect(),
    ))
  }

  async fn call_module_id_impl(
    &mut self,
    module_id: ModuleId,
    function: String,
    args: JsonValue,
  ) -> Result<JsonValue, AnyError> {
    if !args.is_array() {
      return Err(AnyError::msg("args must be a JSON array"));
    }
    let args_json = serde_json::to_string(&args)?;

    let script = format!(
      r#"(async () => {{
  const registry = globalThis.__embedDeno;
  if (!registry) throw new Error('module registry not initialized');
  const id = {id};
  const module = registry.modules.get(id);
  if (!module) throw new Error(`Module not found in registry (id=${{id}})`);
  const funcName = {func:?};
  const fn = module[funcName];
  if (typeof fn !== 'function') throw new Error(`Export '${{funcName}}' is not a function`);
  const args = {args};
  const result = await fn(...args);
  return {{ moduleId: id, result: result ?? null }};
}})();"#,
      id = module_id,
      func = function,
      args = args_json,
    );

    let json = eval_script(&mut self.worker, "[call_module_id]", script).await?;
    let (embed_result, embed_exit_data) = self.take_embed_payload();

    let returned_module_id = json
      .get("moduleId")
      .and_then(|v| v.as_u64())
      .ok_or_else(|| AnyError::msg("call_by_id did not return moduleId"))?
      as ModuleId;
    let result = json.get("result").cloned().unwrap_or(JsonValue::Null);

    Ok(JsonValue::Object(
      [
        ("module_id".to_string(), JsonValue::from(returned_module_id)),
        ("result".to_string(), result),
        (
          "embed_result".to_string(),
          embed_result.unwrap_or(JsonValue::Null),
        ),
        (
          "embed_exit_data".to_string(),
          embed_exit_data.unwrap_or(JsonValue::Null),
        ),
      ]
      .into_iter()
      .collect(),
    ))
  }
}

async fn eval_script(
  worker: &mut MainWorker,
  name: &'static str,
  script: String,
) -> Result<JsonValue, AnyError> {
  let execute_result = worker.execute_script(name, script.into())?;

  // Resolve the promise (if any) and poll event loop.
  let resolve_future = worker.js_runtime.resolve(execute_result);
  let resolve_result = worker
    .js_runtime
    .with_event_loop_future(
      resolve_future,
      PollEventLoopOptions {
        wait_for_inspector: false,
        pump_v8_message_loop: true,
      },
    )
    .await?;

  let result_value = v8_global_to_json_value(&mut worker.js_runtime, resolve_result);

  // Do not run the event loop to completion here.
  //
  // Long-lived modules may keep pending ops indefinitely.
  // We'll keep pumping the event loop in the daemon loop.
  pump_event_loop_once(worker)?;

  Ok(result_value)
}

fn pump_event_loop_once(worker: &mut MainWorker) -> Result<(), AnyError> {
  let waker = noop_waker_ref();
  let mut cx = Context::from_waker(waker);

  match worker.js_runtime.poll_event_loop(
    &mut cx,
    PollEventLoopOptions {
      wait_for_inspector: false,
      pump_v8_message_loop: true,
    },
  ) {
    Poll::Ready(Ok(())) | Poll::Pending => Ok(()),
    Poll::Ready(Err(err)) => Err(AnyError::from(err)),
  }
}

fn v8_global_to_json_value(js_runtime: &mut JsRuntime, value: v8::Global<v8::Value>) -> JsonValue {
  scope!(scope, js_runtime);
  let local = Local::new(scope, value);

  if local.is_null() || local.is_undefined() {
    return JsonValue::Null;
  }

  match serde_v8::from_v8::<JsonValue>(scope, local) {
    Ok(json_value) => json_value,
    Err(err) => JsonValue::String(format!(
      "<non-serializable v8 value: {err}> {}",
      local.to_rust_string_lossy(scope)
    )),
  }
}

fn bootstrap_specifier() -> Result<ModuleSpecifier, AnyError> {
  // This module only exists to bootstrap a worker and keep it in a valid state
  // for subsequent `execute_script` calls.
  let code = "globalThis.__embedDeno = globalThis.__embedDeno ?? { nextModuleId: 1, modules: new \
              Map(), specToId: new Map() }; export {};";
  let encoded = base64::engine::general_purpose::STANDARD.encode(code);
  let spec = format!("data:application/javascript;base64,{encoded}");
  Ok(ModuleSpecifier::parse(&spec)?)
}

fn parse_load_module_result(payload: JsonValue) -> Result<LoadModuleResult, AnyError> {
  let module_id = payload
    .get("module_id")
    .and_then(|v| v.as_u64())
    .ok_or_else(|| AnyError::msg("missing module_id"))? as ModuleId;
  Ok(LoadModuleResult {
    module_id,
    embed_result: payload.get("embed_result").cloned().and_then(not_null),
    embed_exit_data: payload.get("embed_exit_data").cloned().and_then(not_null),
  })
}

fn parse_call_module_result(payload: JsonValue) -> Result<CallModuleResult, AnyError> {
  let module_id = payload
    .get("module_id")
    .and_then(|v| v.as_u64())
    .ok_or_else(|| AnyError::msg("missing module_id"))? as ModuleId;
  let result = payload.get("result").cloned().unwrap_or(JsonValue::Null);
  Ok(CallModuleResult {
    module_id,
    result,
    embed_result: payload.get("embed_result").cloned().and_then(not_null),
    embed_exit_data: payload.get("embed_exit_data").cloned().and_then(not_null),
  })
}

fn not_null(value: JsonValue) -> Option<JsonValue> {
  if value.is_null() { None } else { Some(value) }
}

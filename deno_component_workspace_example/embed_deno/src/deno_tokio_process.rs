use std::{ffi::OsString, path::PathBuf, sync::Arc, time::Instant};

use deno_core::{
  JsRuntime, ModuleSpecifier, PollEventLoopOptions,
  error::AnyError,
  resolve_url_or_path, scope, serde_v8,
  v8::{self, Local, Platform},
};
use deno_lib::{
  npm::create_npm_process_state_provider,
  version,
  worker::{LibMainWorkerFactory, LibWorkerFactoryRoots},
};
use deno_runtime::{WorkerExecutionMode, worker::MainWorker};
use deno_telemetry::OtelConfig;
use deno_terminal::colors;
use log::{error, info, warn};
use serde_json::Value as JsonValue;
use tokio::{
  sync::{Mutex as TokioMutex, mpsc, oneshot},
  time::{Duration, MissedTickBehavior},
};
use uuid::Uuid;

use crate::{
  args::{DenoSubcommand, Flags, get_default_v8_flags},
  factory::CliFactory,
  util::{
    v8::{get_v8_flags_from_env, init_v8_flags},
    watch_env_tracker::{WatchEnvTracker, load_env_variables_from_env_files},
  },
};

/// Request to execute TypeScript code
#[derive(Debug)]
pub struct DenoRequest {
  /// The script to execute
  pub script: String,
  /// Optional request ID for tracking
  pub id: String,
  /// Channel to send the response back
  pub response_tx: oneshot::Sender<DenoResponse>,
}

/// Response from TypeScript execution
#[derive(Debug, Clone)]
pub struct DenoResponse {
  /// Request ID for tracking
  pub id: String,
  pub result: Result<JsonValue, String>,
}

/// Handle for communicating with the Deno runtime
pub struct DenoRuntimeHandle {
  tx: mpsc::UnboundedSender<DenoRequest>,
}

impl DenoRuntimeHandle {
  pub async fn execute(&self, script: String) -> Result<JsonValue, AnyError> {
    let id = Uuid::new_v4().to_string();
    let (response_tx, response_rx) = oneshot::channel();

    info!(
      "📤 Sending request via channel (id: {})， the script is {:?}",
      id, script
    );
    self
      .tx
      .send(DenoRequest {
        script,
        id: id.clone(),
        response_tx,
      })
      .expect("couldn't send on channel");

    info!("⏳ Waiting for response...");
    let response = response_rx.await?;

    // Verify request/response ID match
    if response.id != id {
      return Err(AnyError::msg(format!(
        "Response ID mismatch: expected {}, got {}",
        id, response.id
      )));
    }

    info!("📬 Received response (id: {})", response.id);
    response.result.map_err(|err| AnyError::msg(err))
  }
}

/// Core Deno runtime manager
pub struct DenoRuntimeManager {
  flags: Arc<Flags>,
  start_time: Instant,
  worker: Arc<TokioMutex<MainWorker>>,
}

impl DenoRuntimeManager {
  /// Main entry point - runs daemon with handle
  pub async fn run(self) -> Result<i32, AnyError> {
    let (_handle, daemon_future) = self.run_with_handle().await?;

    // Run daemon until interrupted
    tokio::select! {
      _ = daemon_future => {},
      _ = tokio::signal::ctrl_c() => {},
    }

    Ok(0)
  }
  /// Create a new DenoRuntimeManager from command line arguments
  pub async fn from_args(
    args: Vec<OsString>,
    v8_already_initialized: bool,
  ) -> Result<Self, AnyError> {
    info!("Initializing DenoRuntimeManager from args: {:?}", args);

    // Parse and resolve flags
    let flags = Self::resolve_flags(args).await?;
    let flags_arc = Arc::new(flags);

    // Initialize V8 if not already done
    if !v8_already_initialized {
      Self::init_v8(&flags_arc);
    }

    // Create the main worker using CliFactory
    let worker = Self::create_main_worker(flags_arc.clone()).await?;

    Ok(Self {
      flags: flags_arc,
      start_time: Instant::now(),
      #[allow(clippy::arc_with_non_send_sync)]
      worker: Arc::new(TokioMutex::new(worker)),
    })
  }

  /// Create a MainWorker instance using CliFactory (proper way with all extensions)
  async fn create_main_worker(flags: Arc<Flags>) -> Result<MainWorker, AnyError> {
    let cli_factory = CliFactory::from_flags(flags.clone());
    let cli_options = cli_factory.cli_options()?;

    // Get the module specifier
    let module_specifier = if let DenoSubcommand::Run(run_flags) = &flags.subcommand {
      resolve_url_or_path(&run_flags.script, std::env::current_dir()?.as_path())?
    } else {
      return Err(AnyError::msg("No script specified"));
    };

    let module_loader_factory = cli_factory.create_module_loader_factory().await?;
    cli_factory.maybe_start_inspector_server()?;
    let node_resolver = cli_factory.node_resolver().await?.clone();
    let npm_resolver = cli_factory.npm_resolver().await?;
    let pkg_json_resolver = cli_factory.pkg_json_resolver()?.clone();
    let fs = cli_factory.fs().clone();

    let lib_main_worker_factory = LibMainWorkerFactory::new(
      cli_factory.blob_store().clone(),
      if cli_options.code_cache_enabled() {
        Some(cli_factory.code_cache()?.clone())
      } else {
        None
      },
      None, // DenoRtNativeAddonLoader
      cli_factory.feature_checker()?.clone(),
      fs,
      cli_options.coverage_dir(),
      Box::new(module_loader_factory),
      node_resolver,
      create_npm_process_state_provider(npm_resolver),
      pkg_json_resolver,
      cli_factory.root_cert_store_provider().clone(),
      cli_options.resolve_storage_key_resolver(),
      cli_factory.sys(),
      cli_factory.create_lib_main_worker_options()?,
      LibWorkerFactoryRoots::default(),
      None,
    );

    Ok(
      lib_main_worker_factory
        .create_main_worker(
          WorkerExecutionMode::Run,
          cli_factory.root_permissions_container()?.clone(),
          module_specifier,
          cli_options.preload_modules()?,
          cli_options.require_modules()?,
        )?
        .into_main_worker(),
    )
  }

  /// Initialize the Deno engine by executing the main module
  async fn init_engine(&self, module_url: &ModuleSpecifier) -> Result<(), AnyError> {
    let mut worker = self.worker.lock().await;
    worker.execute_main_module(module_url).await?;
    worker.dispatch_load_event()?;
    info!("Deno engine initialized successfully");
    Ok(())
  }

  /// Execute a script asynchronously and return the result as JSON
  async fn execute_script_async(&self, script: String) -> Result<JsonValue, AnyError> {
    // Check if this is a function call request (format:
    // CALL_FUNCTION:module_path|function_name|args_json)
    if script.starts_with("CALL_FUNCTION:") {
      let parts: Vec<&str> = script
        .trim_start_matches("CALL_FUNCTION:")
        .split('|')
        .collect();
      if parts.len() == 3 {
        let module_path = parts[0].trim().to_string();
        let function_name = parts[1].trim().to_string();
        let args_json = parts[2].trim().to_string();
        return self
          .call_module_function(module_path, function_name, args_json)
          .await;
      } else {
        return Err(AnyError::msg(
          "Invalid CALL_FUNCTION format. Expected: \
           CALL_FUNCTION:module_path|function_name|args_json",
        ));
      }
    }

    // Check if the script contains imports - if so, use module execution
    if script.contains("import ") {
      return self.execute_module_async(script).await;
    }

    // Wrap the script in an async IIFE and capture the result
    // The script can contain statements, so we use a function body
    let wrapped_script = format!(
      r#"
      (async () => {{
        {}
      }})();
      "#,
      script
    );

    let mut worker = self.worker.lock().await;
    let execute_result = worker.execute_script("[execute]", wrapped_script.into())?;

    // Resolve the promise and poll event loop
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

    // Extract the result as JSON
    let result_value = Self::v8_global_to_json_value(&mut worker.js_runtime, resolve_result);

    // IMPORTANT: Continue polling the event loop until all pending operations complete
    // This ensures async operations like fetch() complete even if the script returns early
    info!("Polling event loop to completion to handle remaining async operations...");
    worker
      .js_runtime
      .run_event_loop(PollEventLoopOptions {
        wait_for_inspector: false,
        pump_v8_message_loop: true,
      })
      .await?;

    info!("Script executed successfully, result: {}", result_value);
    Ok(result_value)
  }

  /// Execute module code (with import statements) by writing to temp file and using dynamic import
  async fn execute_module_async(&self, module_code: String) -> Result<JsonValue, AnyError> {
    // Create a temporary file for the module
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("deno_module_{}.ts", uuid::Uuid::new_v4()));

    info!("Writing module to temporary file: {:?}", temp_file);
    std::fs::write(&temp_file, &module_code)?;

    // Convert to file:// URL
    let file_url = format!("file://{}", temp_file.display());

    info!("Executing module via dynamic import from: {}", file_url);

    // Use dynamic import to load and execute the module
    let import_script = format!(
      r#"
      (async () => {{
        try {{
          await import("{}");
          return "Module executed successfully";
        }} catch (e) {{
          throw e;
        }}
      }})();
      "#,
      file_url
    );

    let mut worker = self.worker.lock().await;
    let execute_result = worker.execute_script("[dynamic_import]", import_script.into())?;

    // Resolve the promise and poll event loop
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

    // Extract the result
    let result_value = Self::v8_global_to_json_value(&mut worker.js_runtime, resolve_result);

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_file);

    info!("Module executed successfully, result: {}", result_value);
    Ok(result_value)
  }

  /// Call a specific function from a TypeScript module with arguments
  ///
  /// # Arguments
  /// * `module_path` - Path to the TypeScript module (e.g., "file:///path/to/module.ts")
  /// * `function_name` - Name of the exported function to call
  /// * `args_json` - JSON string containing the arguments array
  ///
  /// # Returns
  /// * `Result<JsonValue, AnyError>` - JSON result from the function call
  async fn call_module_function(
    &self,
    module_path: String,
    function_name: String,
    args_json: String,
  ) -> Result<JsonValue, AnyError> {
    info!(
      "Calling function '{}' from module '{}' with args: {}",
      function_name, module_path, args_json
    );

    // Build the import and function call script
    let call_script = format!(
      r#"
      (async () => {{
        try {{
          const module = await import("{}");
          const func = module["{}"];
          if (typeof func !== 'function') {{
            throw new Error(`Function '{}' not found or is not a function in module`);
          }}
          const args = {};
          const result = await func(...args);
          return result ?? null;
        }} catch (e) {{
          throw e;
        }}
      }})();
      "#,
      module_path, function_name, function_name, args_json
    );

    let mut worker = self.worker.lock().await;
    let execute_result = worker.execute_script("[call_function]", call_script.into())?;

    // Resolve the promise and poll event loop
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

    // Extract the result
    let result_value = Self::v8_global_to_json_value(&mut worker.js_runtime, resolve_result);

    // Continue polling event loop to completion
    info!("Polling event loop to completion...");
    worker
      .js_runtime
      .run_event_loop(PollEventLoopOptions {
        wait_for_inspector: false,
        pump_v8_message_loop: true,
      })
      .await?;

    info!(
      "Function call executed successfully, result: {}",
      result_value
    );
    Ok(result_value)
  }

  /// Poll the Deno event loop
  async fn poll_event_loop(&self) -> Result<(), AnyError> {
    let mut worker = self.worker.lock().await;
    worker
      .js_runtime
      .run_event_loop(PollEventLoopOptions {
        wait_for_inspector: false,
        pump_v8_message_loop: true,
      })
      .await?;
    Ok(())
  }

  /// Wait for the next request from the queue
  async fn recv_request(
    rx: Arc<TokioMutex<mpsc::UnboundedReceiver<DenoRequest>>>,
  ) -> Option<DenoRequest> {
    let mut receiver = rx.lock().await;
    receiver.recv().await
  }

  /// Process a single request from the queue
  async fn process_request(request: DenoRequest, manager: Arc<DenoRuntimeManager>) {
    info!("📨 Received request to execute script (id: {})", request.id);

    let request_id = request.id;
    let script = request.script;
    let response_tx = request.response_tx;

    info!("🔄 Executing script...");
    match manager.execute_script_async(script).await {
      Ok(res) => {
        info!("✅ Script execution completed successfully");
        let _ = response_tx.send(DenoResponse {
          id: request_id,
          result: Ok(res),
        });
      }
      Err(err) => {
        error!("❌ Error executing script: {:?}", err);
        let _ = response_tx.send(DenoResponse {
          id: request_id,
          result: Err(err.to_string()),
        });
      }
    }
  }

  /// Run daemon and return handle for sending requests
  /// Returns (handle, daemon_future) where daemon_future must be polled
  pub async fn run_with_handle(
    self,
  ) -> Result<
    (
      DenoRuntimeHandle,
      impl std::future::Future<Output = ()> + 'static,
    ),
    AnyError,
  > {
    info!("Starting Deno runtime execution");

    // Get the module specifier
    let module_specifier = if let DenoSubcommand::Run(run_flags) = &self.flags.subcommand {
      resolve_url_or_path(&run_flags.script, std::env::current_dir()?.as_path())?
    } else {
      return Err(AnyError::msg("No script specified"));
    };

    // Initialize the engine
    self.init_engine(&module_specifier).await?;

    // Create channels for request/response
    let (tx_outside, rx_inside) = mpsc::unbounded_channel::<DenoRequest>();
    let rx_inside = Arc::new(TokioMutex::new(rx_inside));

    // Store the handle in an Arc for sharing
    let manager_arc = Arc::new(self);

    // Return handle to caller
    let handle = DenoRuntimeHandle { tx: tx_outside };

    // Create daemon future
    let daemon_future = async move {
      Self::run_daemon_loop(rx_inside, manager_arc).await;
    };

    Ok((handle, daemon_future))
  }

  /// Background daemon loop
  async fn run_daemon_loop(
    rx_inside: Arc<TokioMutex<mpsc::UnboundedReceiver<DenoRequest>>>,
    manager_arc: Arc<DenoRuntimeManager>,
  ) {
    // Create heartbeat timer
    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    heartbeat.tick().await;

    let mut heartbeat_count = 0u64;

    info!("Entering daemon loop with tokio::select");

    loop {
      tokio::select! {
        // Process script execution requests as soon as they arrive
        maybe_request = Self::recv_request(rx_inside.clone()) => {
          match maybe_request {
            Some(request) => {
              Self::process_request(request, manager_arc.clone()).await;
            }
            None => {
              info!("Request channel closed. Shutting down daemon loop.");
              break;
            }
          }
        }

        // Heartbeat
        _ = heartbeat.tick() => {
          heartbeat_count += 1;
          info!(
            "Daemon heartbeat #{}, uptime {:?}",
            heartbeat_count,
            manager_arc.start_time.elapsed()
          );
          // Poll event loop periodically during heartbeat
          if let Err(e) = manager_arc.poll_event_loop().await {
            error!("Error polling event loop: {:?}", e);
          }
        }

        // Ctrl+C signal
        _ = tokio::signal::ctrl_c() => {
          info!("Shutdown signal received. Total uptime: {:?}", manager_arc.start_time.elapsed());
          break;
        }
      }
    }
  }

  fn v8_global_to_json_value(
    js_runtime: &mut JsRuntime,
    value: v8::Global<v8::Value>,
  ) -> JsonValue {
    scope!(scope, js_runtime);
    let local = Local::new(scope, value);

    if local.is_null() || local.is_undefined() {
      return JsonValue::Null;
    }

    match serde_v8::from_v8::<JsonValue>(scope, local) {
      Ok(json_value) => json_value,
      Err(err) => {
        warn!(
          "Failed to convert V8 value to JSON: {}. Falling back to string representation.",
          err
        );
        JsonValue::String(local.to_rust_string_lossy(scope))
      }
    }
  }

  /// Parse and resolve command line flags
  async fn resolve_flags(args: Vec<OsString>) -> Result<Flags, AnyError> {
    let _ = args;
    let mut flags = Flags::default();

    // Default to allowing everything for embedding.
    flags.permissions.allow_all = true;

    // Handle environment variables
    if flags.subcommand.watch_flags().is_some() {
      WatchEnvTracker::snapshot();
    }
    let env_file_paths: Option<Vec<PathBuf>> = flags
      .env_file
      .as_ref()
      .map(|files| files.iter().map(PathBuf::from).collect());
    load_env_variables_from_env_files(env_file_paths.as_ref(), flags.log_level);

    flags.unstable_config.fill_with_env();
    if std::env::var("DENO_COMPAT").is_ok() {
      flags.unstable_config.enable_node_compat();
    }
    if flags.node_conditions.is_empty()
      && let Ok(conditions) = std::env::var("DENO_CONDITIONS")
    {
      flags.node_conditions = conditions
        .split(",")
        .map(|c| c.trim().to_string())
        .collect();
    }

    // Initialize logging
    let otel_config = flags.otel_config();
    Self::init_logging(flags.log_level, Some(otel_config.clone()));
    deno_telemetry::init(version::otel_runtime_config(), otel_config.clone())?;

    Ok(flags)
  }

  /// Initialize V8 runtime
  fn init_v8(flags: &Flags) {
    let default_v8_flags = get_default_v8_flags();
    let env_v8_flags = get_v8_flags_from_env();
    let is_single_threaded = env_v8_flags
      .iter()
      .chain(&flags.v8_flags)
      .any(|flag| flag == "--single-threaded");
    init_v8_flags(&default_v8_flags, &flags.v8_flags, env_v8_flags);
    let v8_platform = if is_single_threaded {
      Some(Platform::new_single_threaded(true).make_shared())
    } else {
      None
    };

    JsRuntime::init_platform(v8_platform);
  }

  /// Initialize logging
  fn init_logging(maybe_level: Option<log::Level>, otel_config: Option<OtelConfig>) {
    deno_lib::util::logger::init(deno_lib::util::logger::InitLoggingOptions {
      maybe_level,
      otel_config,
      on_log_start: crate::util::draw_thread::DrawThread::hide,
      on_log_end: crate::util::draw_thread::DrawThread::show,
    })
  }
}

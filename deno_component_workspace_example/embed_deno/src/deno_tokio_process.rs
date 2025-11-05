use std::{ffi::OsString, path::PathBuf, sync::Arc, time::Instant};

use deno_core::{
  JsRuntime, ModuleSpecifier, PollEventLoopOptions,
  error::AnyError,
  resolve_url_or_path, scope,
  v8::{Local, Platform},
};
use deno_lib::{version, worker::LibWorkerFactoryRoots};
use deno_runtime::{WorkerExecutionMode, worker::MainWorker};
use deno_telemetry::OtelConfig;
use deno_terminal::colors;
use log::{error, info};
use tokio::{
  sync::{Mutex as TokioMutex, mpsc, oneshot},
  time::{Duration, MissedTickBehavior},
};
use uuid::Uuid;

use crate::{
  args::{DenoSubcommand, Flags, flags_from_vec, get_default_v8_flags},
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
  pub result: Result<String, String>,
}

/// Handle for communicating with the Deno runtime
pub struct DenoRuntimeHandle {
  tx: mpsc::UnboundedSender<DenoRequest>,
}

impl DenoRuntimeHandle {
  pub async fn execute(&self, script: String) -> Result<String, AnyError> {
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
        id,
        response_tx,
      })
      .expect("couldn't send on channel");

    info!("⏳ Waiting for response...");
    let response = response_rx.await?;
    info!("📬 Received response");
    response.result.map_err(|err| AnyError::msg(err))
  }
}

/// Core Deno runtime manager
pub struct DenoRuntimeManager {
  flags: Arc<Flags>,
  roots: LibWorkerFactoryRoots,
  start_time: Instant,
  worker: Arc<TokioMutex<MainWorker>>,
}

impl DenoRuntimeManager {
  /// Create a new DenoRuntimeManager from command line arguments
  pub async fn from_args(
    args: Vec<OsString>,
    roots: LibWorkerFactoryRoots,
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
    let worker = Self::create_main_worker(flags_arc.clone(), &roots).await?;

    Ok(Self {
      flags: flags_arc,
      roots,
      start_time: Instant::now(),
      #[allow(clippy::arc_with_non_send_sync)]
      worker: Arc::new(TokioMutex::new(worker)),
    })
  }

  /// Create a MainWorker instance using CliFactory (proper way with all extensions)
  async fn create_main_worker(
    flags: Arc<Flags>,
    _roots: &LibWorkerFactoryRoots,
  ) -> Result<MainWorker, AnyError> {
    // Create CliFactory
    let cli_factory = CliFactory::from_flags(flags.clone());

    // Create worker factory with proper extensions
    let worker_factory = cli_factory.create_cli_main_worker_factory().await?;

    // Get the module specifier
    let module_specifier = if let DenoSubcommand::Run(run_flags) = &flags.subcommand {
      resolve_url_or_path(&run_flags.script, std::env::current_dir()?.as_path())?
    } else {
      return Err(AnyError::msg("No script specified"));
    };

    // Create the worker with empty side module list and convert to MainWorker
    let cli_worker = worker_factory
      .create_main_worker(WorkerExecutionMode::Run, module_specifier, vec![])
      .await?;

    // Convert CliMainWorker to MainWorker
    Ok(cli_worker.into_main_worker())
  }

  /// Initialize the Deno engine by executing the main module
  async fn init_engine(&self, module_url: &ModuleSpecifier) -> Result<(), AnyError> {
    let mut worker = self.worker.lock().await;
    worker.execute_main_module(module_url).await?;
    worker.dispatch_load_event()?;
    info!("Deno engine initialized successfully");
    Ok(())
  }

  /// Execute a script asynchronously and return the result as a string
  async fn execute_script_async(&self, script: String) -> Result<String, AnyError> {
    // Wrap the script in an async IIFE and stringify the result
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

    // Extract the stringified result from V8 Global
    let result_str = {
      scope!(scope, &mut worker.js_runtime);
      let local = Local::new(scope, resolve_result);
      local.to_rust_string_lossy(scope)
    };

    info!("Script executed successfully, result: {}", result_str);
    Ok(result_str)
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

  /// Process a single request from the queue
  async fn process_request(
    rx: Arc<TokioMutex<mpsc::UnboundedReceiver<DenoRequest>>>,
    manager: Arc<DenoRuntimeManager>,
  ) {
    let mut receiver = rx.lock().await;
    if let Ok(request) = receiver.try_recv() {
      info!("📨 Received request to execute script");
      drop(receiver); // Release lock immediately

      let script = request.script;
      let response_tx = request.response_tx;

      info!("🔄 Executing script...");
      match manager.execute_script_async(script).await {
        Ok(res) => {
          info!("✅ Script execution completed successfully");
          let _ = response_tx.send(DenoResponse { result: Ok(res) });
        }
        Err(err) => {
          error!("❌ Error executing script: {:?}", err);
          let _ = response_tx.send(DenoResponse {
            result: Err(err.to_string()),
          });
        }
      }
    }
  }

  /// Main entry point - runs daemon with handle
  pub async fn run(self) -> Result<i32, AnyError> {
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

    // Clone tx for testing
    let handle = DenoRuntimeHandle { tx: tx_outside };

    // Send a test script execution request
    println!("Sending test script execution request...");
    let test_script = r#"
      const result = {
        message: "Hello from TypeScript!",
        timestamp: new Date().toISOString(),
        env: {
          api_key: Deno.env.get("STREAM_API_KEY"),
          api_secret: Deno.env.get("STREAM_API_SECRET")
        }
      };
      return JSON.stringify(result);
    "#
    .to_string();

    let handle_clone = DenoRuntimeHandle {
      tx: handle.tx.clone(),
    };
    tokio::spawn(async move {
      tokio::time::sleep(Duration::from_secs(2)).await;
      match handle_clone.execute(test_script).await {
        Ok(result) => {
          println!("✅ Script execution result:");
          println!("{}", result);
        }
        Err(err) => {
          eprintln!("❌ Script execution failed: {}", err);
        }
      }
    });

    // Run the daemon loop
    Self::run_daemon_loop(rx_inside, manager_arc).await;

    Ok(0)
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

    // Create a ticker for processing requests
    let mut request_ticker = tokio::time::interval(Duration::from_millis(10));
    request_ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut heartbeat_count = 0u64;

    info!("Entering daemon loop with tokio::select");

    loop {
      tokio::select! {
        // Process script execution requests
        // Note: We don't poll event loop separately because execute_script_async
        // handles event loop polling internally through resolve()
        _ = request_ticker.tick() => {
          Self::process_request(rx_inside.clone(), manager_arc.clone()).await;
        }

        // Heartbeat
        _ = heartbeat.tick() => {
          heartbeat_count += 1;
          info!(
            "Daemon heartbeat #{}, uptime {:?}",
            heartbeat_count,
            manager_arc.start_time.elapsed()
          );
        }

        // Ctrl+C signal
        _ = tokio::signal::ctrl_c() => {
          info!("Shutdown signal received. Total uptime: {:?}", manager_arc.start_time.elapsed());
          break;
        }
      }
    }
  }

  /// Parse and resolve command line flags
  async fn resolve_flags(args: Vec<OsString>) -> Result<Flags, AnyError> {
    let mut flags = match flags_from_vec(args) {
      Ok(flags) => flags,
      Err(err @ clap::Error { .. }) if err.kind() == clap::error::ErrorKind::DisplayVersion => {
        let _ = err.print();
        std::process::exit(0);
      }
      Err(err) => {
        let error_string = format!("{err:?}");
        error!(
          "{}: {}",
          colors::red_bold("error"),
          error_string.trim_start_matches("error: ")
        );
        return Err(AnyError::from(err));
      }
    };

    // Set default permissions
    if !flags.permissions.allow_all
      && flags.permissions.allow_read.is_none()
      && flags.permissions.allow_write.is_none()
      && flags.permissions.allow_net.is_none()
      && flags.permissions.allow_env.is_none()
      && flags.permissions.allow_run.is_none()
    {
      flags.permissions.allow_all = true;
      flags.permissions.allow_read = Some(vec![]);
      flags.permissions.allow_write = Some(vec![]);
      flags.permissions.allow_net = Some(vec![]);
      flags.permissions.allow_env = Some(vec![]);
      flags.permissions.allow_run = Some(vec![]);
      flags.permissions.allow_ffi = Some(vec![]);
      flags.permissions.allow_sys = Some(vec![]);
    }

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

    JsRuntime::init_platform(v8_platform, false);
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

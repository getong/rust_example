use std::{collections::HashMap, ffi::OsString, path::PathBuf, sync::Arc, time::Instant};

use deno_core::{
  ModuleSpecifier,
  anyhow::{Context, Error},
  error::AnyError,
};
use deno_lib::{util::result::js_error_downcast_ref, worker::LibWorkerFactoryRoots};
use deno_runtime::{WorkerExecutionMode, fmt_errors::format_js_error, worker::MainWorker};
use deno_telemetry::OtelConfig;
use deno_terminal::colors;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use tokio::{
  net::UnixStream,
  sync::{mpsc, oneshot},
  time::{Duration, MissedTickBehavior},
};

use crate::{
  args::{DenoSubcommand, Flags, flags_from_vec, get_default_v8_flags},
  factory::CliFactory,
  util::{
    v8::{get_v8_flags_from_env, init_v8_flags},
    watch_env_tracker::{WatchEnvTracker, load_env_variables_from_env_files},
  },
};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum UserWorkerMsgs {
  Shutdown,
}

pub type EnvVars = HashMap<String, String>;

/// Execution result events inspired by edge-runtime architecture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionEvent {
  Success {
    cpu_time_ms: u64,
    wall_time_ms: u64,
  },
  Error {
    exception: String,
    cpu_time_ms: u64,
    wall_time_ms: u64,
  },
  Terminated {
    reason: String,
    cpu_time_ms: u64,
  },
}

/// Result channel for execution events
pub type ExecutionEventSender = oneshot::Sender<ExecutionEvent>;
pub type ExecutionEventReceiver = oneshot::Receiver<ExecutionEvent>;

/// TypeScript command execution context
#[derive(Debug, Clone)]
pub struct ExecutionContext {
  pub command: String,
  pub args: Vec<String>,
  pub env_vars: HashMap<String, String>,
}

pub struct DenoTokioProcess<'a> {
  worker: &'a mut MainWorker,
  main_module_url: ModuleSpecifier,
  worker_pool_tx: Option<mpsc::UnboundedSender<UserWorkerMsgs>>,
  execution_event_tx: Option<ExecutionEventSender>,
}

impl<'a> DenoTokioProcess<'a> {
  pub fn new(
    worker: &'a mut MainWorker,
    main_module_url: ModuleSpecifier,
    worker_pool_tx: Option<mpsc::UnboundedSender<UserWorkerMsgs>>,
  ) -> Self {
    Self {
      worker,
      main_module_url,
      worker_pool_tx,
      execution_event_tx: None,
    }
  }

  /// Create a new process with execution event channel
  pub fn new_with_events(
    worker: &'a mut MainWorker,
    main_module_url: ModuleSpecifier,
    worker_pool_tx: Option<mpsc::UnboundedSender<UserWorkerMsgs>>,
    execution_event_tx: ExecutionEventSender,
  ) -> Self {
    Self {
      worker,
      main_module_url,
      worker_pool_tx,
      execution_event_tx: Some(execution_event_tx),
    }
  }

  pub async fn run(
    mut self,
    stream: UnixStream,
    shutdown_tx: oneshot::Sender<()>,
  ) -> Result<(), Error> {
    let wall_start = Instant::now();
    let cpu_start = Self::get_cpu_time_ms();

    let (unix_stream_tx, unix_stream_rx) = mpsc::unbounded_channel::<UnixStream>();
    unix_stream_tx
      .send(stream)
      .map_err(|err| Error::msg(format!("failed to forward unix stream: {err:?}")))?;

    let env_vars: EnvVars = std::env::vars().collect();
    {
      let op_state_rc = self.worker.js_runtime.op_state();
      let mut op_state = op_state_rc.borrow_mut();
      op_state.put::<mpsc::UnboundedReceiver<UnixStream>>(unix_stream_rx);
      if let Some(worker_pool_tx) = self.worker_pool_tx.clone() {
        op_state.put::<mpsc::UnboundedSender<UserWorkerMsgs>>(worker_pool_tx);
      }
      op_state.put::<EnvVars>(env_vars);
    }

    let run_result = async {
      self
        .worker
        .execute_main_module(&self.main_module_url)
        .await
        .context("failed to execute main module")?;
      self
        .worker
        .dispatch_load_event()
        .context("failed to dispatch load event")?;

      tokio::select! {
        run = self.worker.run_event_loop(false) => {
          debug!("deno tokio process event loop completed");
          run.context("event loop execution failed")?
        }
      }

      Ok::<(), Error>(())
    }
    .await;

    // Calculate execution metrics
    let cpu_time_ms = Self::get_cpu_time_ms() - cpu_start;
    let wall_time_ms = wall_start.elapsed().as_millis() as u64;

    // Send execution event if channel is available
    if let Some(event_tx) = self.execution_event_tx.take() {
      let event = match &run_result {
        Ok(_) => {
          info!(
            "TypeScript execution completed successfully (CPU: {}ms, Wall: {}ms)",
            cpu_time_ms, wall_time_ms
          );
          ExecutionEvent::Success {
            cpu_time_ms,
            wall_time_ms,
          }
        }
        Err(err) => {
          error!(
            "TypeScript execution failed: {err:?} (CPU: {}ms, Wall: {}ms)",
            cpu_time_ms, wall_time_ms
          );
          ExecutionEvent::Error {
            exception: format!("{err:#}"),
            cpu_time_ms,
            wall_time_ms,
          }
        }
      };

      let _ = event_tx.send(event);
    }

    if let Err(err) = &run_result {
      error!("deno tokio process encountered an error: {err:?}");
    }

    let _ = shutdown_tx.send(());

    run_result
  }

  /// Execute a TypeScript command and return the result
  /// This is a higher-level API for running TypeScript code with result capture
  pub async fn execute_command(
    mut self,
    context: ExecutionContext,
    result_tx: oneshot::Sender<Result<String, String>>,
  ) -> Result<(), Error> {
    let wall_start = Instant::now();
    let cpu_start = Self::get_cpu_time_ms();

    info!(
      "Executing TypeScript command: {} with args: {:?}",
      context.command, context.args
    );

    // Set environment variables from context
    {
      let op_state_rc = self.worker.js_runtime.op_state();
      let mut op_state = op_state_rc.borrow_mut();
      op_state.put::<EnvVars>(context.env_vars);
    }

    let run_result = async {
      self
        .worker
        .execute_main_module(&self.main_module_url)
        .await
        .context("failed to execute main module")?;
      self
        .worker
        .dispatch_load_event()
        .context("failed to dispatch load event")?;

      tokio::select! {
        run = self.worker.run_event_loop(false) => {
          debug!("TypeScript command event loop completed");
          run.context("event loop execution failed")?
        }
      }

      Ok::<(), Error>(())
    }
    .await;

    let cpu_time_ms = Self::get_cpu_time_ms() - cpu_start;
    let wall_time_ms = wall_start.elapsed().as_millis() as u64;

    // Send result back through the channel
    let send_result = match run_result {
      Ok(_) => {
        let success_msg = format!(
          "Command executed successfully (CPU: {}ms, Wall: {}ms)",
          cpu_time_ms, wall_time_ms
        );
        info!("{}", success_msg);
        result_tx.send(Ok(success_msg))
      }
      Err(ref err) => {
        let error_msg = format!(
          "Command failed: {err:#} (CPU: {}ms, Wall: {}ms)",
          cpu_time_ms, wall_time_ms
        );
        error!("{}", error_msg);
        result_tx.send(Err(error_msg))
      }
    };

    if send_result.is_err() {
      error!("Failed to send execution result - receiver dropped");
    }

    run_result
  }

  /// Get CPU time in milliseconds (platform-specific implementation)
  fn get_cpu_time_ms() -> u64 {
    #[cfg(target_os = "linux")]
    {
      use std::fs;
      if let Ok(stat) = fs::read_to_string("/proc/self/stat") {
        if let Some(parts) = stat.split_whitespace().nth(13) {
          if let Ok(ticks) = parts.parse::<u64>() {
            // Convert ticks to milliseconds (usually 100 ticks per second)
            return ticks * 10;
          }
        }
      }
      0
    }

    #[cfg(not(target_os = "linux"))]
    {
      // Fallback for non-Linux platforms
      0
    }
  }
}

/// High-level entry point for running TypeScript files from command line arguments
/// This function integrates the main.rs logic for parsing args and executing scripts
pub async fn run_typescript_file(
  args: Vec<OsString>,
  roots: LibWorkerFactoryRoots,
  v8_already_initialized: bool,
) -> Result<i32, AnyError> {
  info!("Running TypeScript file with args: {:?}", args);

  // Parse command line flags
  let flags = resolve_flags(args).await?;

  // Initialize V8 if not already done
  if !v8_already_initialized {
    init_v8(&flags);
  }

  // Execute the script based on the subcommand
  let exit_code = match flags.subcommand.clone() {
    DenoSubcommand::Run(run_flags) => {
      if run_flags.is_stdin() {
        // Handle stdin input
        crate::tools::run::run_from_stdin(Arc::new(flags), None, roots).await?
      } else {
        // Run the script file
        info!("Executing script: {:?}", run_flags.script);
        crate::tools::run::run_script(
          WorkerExecutionMode::Run,
          Arc::new(flags),
          run_flags.watch,
          None,
          roots,
        )
        .await?
      }
    }
    _ => {
      return Err(AnyError::msg(
        "Only 'run' command is supported in this build",
      ));
    }
  };

  info!(
    "TypeScript file execution completed with exit code: {}",
    exit_code
  );
  Ok(exit_code)
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
    Some(::deno_core::v8::Platform::new_single_threaded(true).make_shared())
  } else {
    None
  };

  deno_core::JsRuntime::init_platform(v8_platform, false);
}

/// Initialize logging with optional level and otel config
fn init_logging(maybe_level: Option<log::Level>, otel_config: Option<OtelConfig>) {
  deno_lib::util::logger::init(deno_lib::util::logger::InitLoggingOptions {
    maybe_level,
    otel_config,
    on_log_start: crate::util::draw_thread::DrawThread::hide,
    on_log_end: crate::util::draw_thread::DrawThread::show,
  })
}

/// Parse and resolve command line flags
async fn resolve_flags(args: Vec<OsString>) -> Result<Flags, AnyError> {
  let mut flags = match flags_from_vec(args) {
    Ok(flags) => flags,
    Err(err @ clap::Error { .. }) if err.kind() == clap::error::ErrorKind::DisplayVersion => {
      // Ignore results to avoid BrokenPipe errors.
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

  // Set default permissions for embedded Deno runtime if no explicit permissions were provided
  if !flags.permissions.allow_all
    && flags.permissions.allow_read.is_none()
    && flags.permissions.allow_write.is_none()
    && flags.permissions.allow_net.is_none()
    && flags.permissions.allow_env.is_none()
    && flags.permissions.allow_run.is_none()
  {
    flags.permissions.allow_all = true;
    flags.permissions.allow_read = Some(vec![]); // Empty vec means allow all
    flags.permissions.allow_write = Some(vec![]); // Empty vec means allow all
    flags.permissions.allow_net = Some(vec![]); // Empty vec means allow all
    flags.permissions.allow_env = Some(vec![]); // Empty vec means allow all
    flags.permissions.allow_run = Some(vec![]); // Empty vec means allow all
    flags.permissions.allow_ffi = Some(vec![]); // Empty vec means allow all
    flags.permissions.allow_sys = Some(vec![]); // Empty vec means allow all
  }

  // Handle environment variables and configuration
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

  // Initialize logging and telemetry
  let otel_config = flags.otel_config();
  init_logging(flags.log_level, Some(otel_config.clone()));
  deno_telemetry::init(
    deno_lib::version::otel_runtime_config(),
    otel_config.clone(),
  )?;

  Ok(flags)
}

/// Main entry point that runs TypeScript file first, then continues with daemon mode
/// This is the recommended entry point for the embed_deno binary
pub async fn run_with_daemon_mode(
  args: Vec<OsString>,
  roots: LibWorkerFactoryRoots,
  v8_already_initialized: bool,
) -> Result<i32, AnyError> {
  info!("Starting embed_deno with daemon mode");

  // Step 1: Execute the TypeScript file
  let exit_code = run_typescript_file(args, roots, v8_already_initialized).await?;

  info!(
    "TypeScript execution completed with exit code: {}",
    exit_code
  );

  // Step 2: If successful, run in daemon mode
  if exit_code == 0 {
    info!("Deno script completed successfully. Entering daemon mode...");
    run_daemon_loop(exit_code).await;
  }

  Ok(exit_code)
}

/// Run the daemon heartbeat loop
/// This keeps the process alive and logs periodic heartbeats
async fn run_daemon_loop(exit_code: i32) -> ! {
  let start_time = std::time::Instant::now();
  let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
  heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
  heartbeat.tick().await; // First tick completes immediately

  let mut heartbeat_count = 0u64;

  loop {
    tokio::select! {
      result = tokio::signal::ctrl_c() => {
        match result {
          Ok(_) => {
            println!("\n🛑 Daemon shutdown requested (Ctrl+C). Total uptime: {:?}", start_time.elapsed());
          }
          Err(err) => {
            eprintln!("⚠️  Error listening for Ctrl+C: {}", err);
          }
        }
        break;
      }
      _ = heartbeat.tick() => {
        heartbeat_count += 1;
        println!(
          "💤 Daemon heartbeat #{}, uptime {:?}",
          heartbeat_count,
          start_time.elapsed()
        );
      }
    }
  }

  deno_runtime::exit(exit_code);
}

/// Deno Runtime Manager - encapsulates the entire lifecycle of running TypeScript
/// and managing the daemon process
pub struct DenoRuntimeManager {
  flags: Flags,
  roots: LibWorkerFactoryRoots,
  start_time: Instant,
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

    // Initialize V8 if not already done
    if !v8_already_initialized {
      Self::init_v8(&flags);
    }

    Ok(Self {
      flags,
      roots,
      start_time: Instant::now(),
    })
  }

  /// Main entry point - executes TypeScript and then runs daemon loop
  pub async fn run(self) -> Result<i32, AnyError> {
    info!("Starting Deno runtime execution");

    // Step 1: Execute the TypeScript file
    let exit_code = self.execute_typescript().await?;

    info!(
      "TypeScript execution completed with exit code: {}",
      exit_code
    );

    // Step 2: If successful, run in daemon mode
    if exit_code == 0 {
      info!("Deno script completed successfully. Entering daemon mode...");
      self.run_daemon_loop(exit_code).await;
    }

    Ok(exit_code)
  }

  /// Execute the TypeScript file based on the subcommand
  async fn execute_typescript(&self) -> Result<i32, AnyError> {
    let exit_code = match self.flags.subcommand.clone() {
      DenoSubcommand::Run(run_flags) => {
        if run_flags.is_stdin() {
          // Handle stdin input
          crate::tools::run::run_from_stdin(Arc::new(self.flags.clone()), None, self.roots.clone())
            .await?
        } else {
          // Run the script file
          info!("Executing script: {:?}", run_flags.script);
          crate::tools::run::run_script(
            WorkerExecutionMode::Run,
            Arc::new(self.flags.clone()),
            run_flags.watch,
            None,
            self.roots.clone(),
          )
          .await?
        }
      }
      _ => {
        return Err(AnyError::msg(
          "Only 'run' command is supported in this build",
        ));
      }
    };

    Ok(exit_code)
  }

  /// Run the daemon heartbeat loop
  /// This keeps the process alive and logs periodic heartbeats
  async fn run_daemon_loop(self, exit_code: i32) -> ! {
    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    heartbeat.tick().await; // First tick completes immediately

    let mut heartbeat_count = 0u64;

    loop {
      tokio::select! {
        result = tokio::signal::ctrl_c() => {
          match result {
            Ok(_) => {
              println!("\n🛑 Daemon shutdown requested (Ctrl+C). Total uptime: {:?}", self.start_time.elapsed());
            }
            Err(err) => {
              eprintln!("⚠️  Error listening for Ctrl+C: {}", err);
            }
          }
          break;
        }
        _ = heartbeat.tick() => {
          heartbeat_count += 1;
          println!(
            "💤 Daemon heartbeat #{}, uptime {:?}",
            heartbeat_count,
            self.start_time.elapsed()
          );
        }
      }
    }

    deno_runtime::exit(exit_code);
  }

  /// Parse and resolve command line flags
  async fn resolve_flags(args: Vec<OsString>) -> Result<Flags, AnyError> {
    let mut flags = match flags_from_vec(args) {
      Ok(flags) => flags,
      Err(err @ clap::Error { .. }) if err.kind() == clap::error::ErrorKind::DisplayVersion => {
        // Ignore results to avoid BrokenPipe errors.
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

    // Set default permissions for embedded Deno runtime if no explicit permissions were provided
    if !flags.permissions.allow_all
      && flags.permissions.allow_read.is_none()
      && flags.permissions.allow_write.is_none()
      && flags.permissions.allow_net.is_none()
      && flags.permissions.allow_env.is_none()
      && flags.permissions.allow_run.is_none()
    {
      flags.permissions.allow_all = true;
      flags.permissions.allow_read = Some(vec![]); // Empty vec means allow all
      flags.permissions.allow_write = Some(vec![]); // Empty vec means allow all
      flags.permissions.allow_net = Some(vec![]); // Empty vec means allow all
      flags.permissions.allow_env = Some(vec![]); // Empty vec means allow all
      flags.permissions.allow_run = Some(vec![]); // Empty vec means allow all
      flags.permissions.allow_ffi = Some(vec![]); // Empty vec means allow all
      flags.permissions.allow_sys = Some(vec![]); // Empty vec means allow all
    }

    // Handle environment variables and configuration
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

    // Initialize logging and telemetry
    let otel_config = flags.otel_config();
    Self::init_logging(flags.log_level, Some(otel_config.clone()));
    deno_telemetry::init(
      deno_lib::version::otel_runtime_config(),
      otel_config.clone(),
    )?;

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
      Some(::deno_core::v8::Platform::new_single_threaded(true).make_shared())
    } else {
      None
    };

    deno_core::JsRuntime::init_platform(v8_platform, false);
  }

  /// Initialize logging with optional level and otel config
  fn init_logging(maybe_level: Option<log::Level>, otel_config: Option<OtelConfig>) {
    deno_lib::util::logger::init(deno_lib::util::logger::InitLoggingOptions {
      maybe_level,
      otel_config,
      on_log_start: crate::util::draw_thread::DrawThread::hide,
      on_log_end: crate::util::draw_thread::DrawThread::show,
    })
  }
}

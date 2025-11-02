use std::{ffi::OsString, path::PathBuf, sync::Arc, time::Instant};

use deno_core::error::AnyError;
use deno_lib::worker::LibWorkerFactoryRoots;
use deno_runtime::WorkerExecutionMode;
use deno_telemetry::OtelConfig;
use deno_terminal::colors;
use log::{error, info};
use tokio::time::{Duration, MissedTickBehavior};

use crate::{
  args::{DenoSubcommand, Flags, flags_from_vec, get_default_v8_flags},
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

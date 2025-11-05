// Copyright 2018-2025 the Deno authors. MIT license.

mod args;
mod cache;
mod cdp;
mod deno_tokio_process;
mod factory;
mod file_fetcher;
mod graph_container;
mod graph_util;
mod http_util;
mod jsr;
mod lsp;
mod module_loader;
mod node;
mod npm;
mod ops;
mod registry;
mod resolver;
mod standalone;
mod task_runner;
mod tools;
mod tsc;
mod type_checker;
mod util;
mod worker;

pub mod sys {
  #[allow(clippy::disallowed_types)] // ok, definition
  pub type CliSys = sys_traits::impls::RealSys;
}

use std::{env, ffi::OsString, path::PathBuf, sync::Arc};

use deno_core::error::AnyError;
use deno_lib::{util::result::js_error_downcast_ref, worker::LibWorkerFactoryRoots};
use deno_runtime::{
  UnconfiguredRuntime, fmt_errors::format_js_error,
  tokio_util::create_and_run_current_thread_with_maybe_metrics,
};
use deno_telemetry::OtelConfig;
use deno_terminal::colors;
use factory::CliFactory;

use self::util::draw_thread::DrawThread;
use crate::{
  args::{Flags, flags_from_vec, get_default_v8_flags},
  util::{
    display,
    v8::{get_v8_flags_from_env, init_v8_flags},
    watch_env_tracker::{WatchEnvTracker, load_env_variables_from_env_files},
  },
};

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// Ensures that all subcommands return an i32 exit code and an [`AnyError`] error type.
trait SubcommandOutput {
  fn output(self) -> Result<i32, AnyError>;
}

impl SubcommandOutput for Result<i32, AnyError> {
  fn output(self) -> Result<i32, AnyError> {
    self
  }
}

impl SubcommandOutput for Result<(), AnyError> {
  fn output(self) -> Result<i32, AnyError> {
    self.map(|_| 0)
  }
}

use deno_core::{futures::FutureExt, unsync::JoinHandle};

/// Ensure that the subcommand runs in a task, rather than being directly executed.
#[inline(always)]
fn spawn_subcommand<F, T>(f: F) -> JoinHandle<Result<i32, AnyError>>
where
  F: std::future::Future<Output = T> + 'static,
  T: SubcommandOutput,
{
  deno_core::unsync::spawn(async move { f.map(|r| r.output()).await }.boxed_local())
}

async fn run_subcommand(
  _flags: Arc<Flags>,
  _unconfigured_runtime: Option<deno_runtime::UnconfiguredRuntime>,
  _roots: LibWorkerFactoryRoots,
) -> Result<i32, AnyError> {
  // This function is now deprecated - the logic has been moved to
  // deno_tokio_process::run_typescript_file It should not be called anymore, but we keep it for
  // compatibility
  Err(AnyError::msg(
    "run_subcommand is deprecated - use deno_tokio_process::run_typescript_file instead",
  ))
}

#[allow(clippy::print_stderr)]
fn setup_panic_hook() {
  let orig_hook = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |panic_info| {
    eprintln!("\nDeno runtime panicked");
    orig_hook(panic_info);
    deno_runtime::exit(1);
  }));
}

fn exit_with_message(message: &str, code: i32) -> ! {
  log::error!(
    "{}: {}",
    colors::red_bold("error"),
    message.trim_start_matches("error: ")
  );
  deno_runtime::exit(code);
}

fn exit_for_error(error: AnyError) -> ! {
  let error_string = match js_error_downcast_ref(&error) {
    Some(e) => format_js_error(e),
    None => format!("{error:?}"),
  };

  exit_with_message(&error_string, 1);
}

pub(crate) fn unstable_exit_cb(feature: &str, api_name: &str) {
  log::error!(
    "Unstable API '{api_name}'. The `--unstable-{}` flag must be provided.",
    feature
  );
  deno_runtime::exit(70);
}

fn maybe_setup_permission_broker() {
  let Ok(socket_path) = std::env::var("DENO_PERMISSION_BROKER_PATH") else {
    return;
  };
  log::warn!(
    "{} Permission broker is an experimental feature",
    colors::yellow("Warning")
  );
  let broker = deno_runtime::deno_permissions::broker::PermissionBroker::new(socket_path);
  deno_runtime::deno_permissions::broker::set_broker(broker);
}

use tokio::time::{Duration, MissedTickBehavior};
use tokio_util::sync::CancellationToken;

fn run_daemon_mode(exit_code: i32) -> ! {
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .thread_name("deno-daemon")
    .build()
    .unwrap();

  let local = tokio::task::LocalSet::new();

  local.block_on(&runtime, async {
    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();

    tokio::task::spawn_local(async move {
      if let Err(err) = tokio::signal::ctrl_c().await {
        eprintln!("⚠️  Error listening for Ctrl+C: {}", err);
      }
      cancel_token_clone.cancel();
    });

    let start_time = std::time::Instant::now();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    heartbeat.tick().await;

    let mut heartbeat_count = 0u64;

    loop {
      tokio::select! {
        _ = cancel_token.cancelled() => {
          println!("\n🛑 Daemon shutdown requested. Total uptime: {:?}", start_time.elapsed());
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
  });

  deno_runtime::exit(exit_code);
}

pub fn main() {
  #[cfg(feature = "dhat-heap")]
  let profiler = dhat::Profiler::new_heap();

  setup_panic_hook();

  init_logging(None, None);

  util::unix::raise_fd_limit();
  util::windows::ensure_stdio_open();
  #[cfg(windows)]
  {
    deno_subprocess_windows::disable_stdio_inheritance();
    colors::enable_ansi(); // For Windows 10
  }
  deno_runtime::deno_permissions::prompter::set_prompt_callbacks(
    Box::new(util::draw_thread::DrawThread::hide),
    Box::new(util::draw_thread::DrawThread::show),
  );

  maybe_setup_permission_broker();

  rustls::crypto::aws_lc_rs::default_provider()
    .install_default()
    .unwrap();

  let args: Vec<OsString> = env::args_os().collect();

  let future = async move {
    let roots = LibWorkerFactoryRoots::default();

    #[cfg(unix)]
    let (waited_unconfigured_runtime, waited_args) = match wait_for_start(&args, roots.clone()) {
      Some(f) => match f.await {
        Ok(v) => match v {
          Some((u, a)) => (Some(u), Some(a)),
          None => (None, None),
        },
        Err(e) => {
          panic!("Failure from control sock: {e}");
        }
      },
      None => (None, None),
    };

    #[cfg(not(unix))]
    let (waited_unconfigured_runtime, waited_args) = (None, None);

    let args = waited_args.unwrap_or(args);

    // Check if V8 was already initialized in wait_for_start
    let v8_already_initialized = waited_unconfigured_runtime.is_some();

    // Create and run the Deno Runtime Manager
    // This will handle:
    // 1. Flag parsing and V8 initialization
    // 2. TypeScript file execution
    // 3. Daemon mode with heartbeat loop
    // 4. Message passing for script execution
    let runtime_manager =
      crate::deno_tokio_process::DenoRuntimeManager::from_args(args, v8_already_initialized)
        .await?;

    // Get handle and daemon future for sending requests
    let (handle, daemon_future) = runtime_manager.run_with_handle().await?;

    // Read stream.ts file and execute it
    let stream_ts_path = std::env::current_dir()?.join("stream.ts");
    println!("📖 Reading stream.ts file from: {:?}", stream_ts_path);

    let stream_script = match std::fs::read_to_string(&stream_ts_path) {
      Ok(content) => {
        println!(
          "✅ Successfully read stream.ts file ({} bytes)",
          content.len()
        );
        content
      }
      Err(err) => {
        eprintln!("❌ Failed to read stream.ts file: {}", err);
        eprintln!("   Using fallback test script instead");
        // Fallback to test script
        r#"
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
        .to_string()
      }
    };

    // Create a future for script execution
    let script_execution = async {
      // Give daemon time to start
      tokio::time::sleep(std::time::Duration::from_millis(100)).await;

      // Send the script for execution
      println!("📤 Sending script for execution...");
      match handle.execute(stream_script).await {
        Ok(result) => {
          println!("✅ Execution result:");
          println!("{}", result);
        }
        Err(err) => {
          eprintln!("❌ Execution failed: {}", err);
        }
      }

      // Keep running until interrupted
      tokio::signal::ctrl_c().await.ok();
    };

    // Run both the daemon and script execution concurrently
    tokio::select! {
      _ = daemon_future => {},
      _ = script_execution => {},
    }

    Ok(0)
  };

  let result = create_and_run_current_thread_with_maybe_metrics(future);

  #[cfg(feature = "dhat-heap")]
  drop(profiler);

  match result {
    Ok(exit_code) => {
      // The daemon mode is now handled inside run_with_daemon_mode
      // So we just exit with the code
      deno_runtime::exit(exit_code);
    }
    Err(err) => exit_for_error(err),
  }
}

async fn resolve_flags_and_init(args: Vec<std::ffi::OsString>) -> Result<Flags, AnyError> {
  let mut flags = match flags_from_vec(args) {
    Ok(flags) => flags,
    Err(err @ clap::Error { .. }) if err.kind() == clap::error::ErrorKind::DisplayVersion => {
      // Ignore results to avoid BrokenPipe errors.
      let _ = err.print();
      deno_runtime::exit(0);
    }
    Err(err) => exit_for_error(AnyError::from(err)),
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
  // preserve already loaded env variables
  if flags.subcommand.watch_flags().is_some() {
    WatchEnvTracker::snapshot();
  }
  let env_file_paths: Option<Vec<std::path::PathBuf>> = flags
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

  let otel_config = flags.otel_config();
  init_logging(flags.log_level, Some(otel_config.clone()));
  deno_telemetry::init(
    deno_lib::version::otel_runtime_config(),
    otel_config.clone(),
  )?;

  Ok(flags)
}

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

  // TODO(bartlomieju): remove last argument once Deploy no longer needs it
  deno_core::JsRuntime::init_platform(v8_platform, /* import assertions enabled */ false);
}

fn init_logging(maybe_level: Option<log::Level>, otel_config: Option<OtelConfig>) {
  deno_lib::util::logger::init(deno_lib::util::logger::InitLoggingOptions {
    maybe_level,
    otel_config,
    on_log_start: DrawThread::hide,
    on_log_end: DrawThread::show,
  })
}

#[cfg(unix)]
#[allow(clippy::type_complexity)]
fn wait_for_start(
  args: &[std::ffi::OsString],
  roots: LibWorkerFactoryRoots,
) -> Option<
  impl std::future::Future<
    Output = Result<Option<(UnconfiguredRuntime, Vec<std::ffi::OsString>)>, AnyError>,
  > + use<>,
> {
  let startup_snapshot = deno_snapshots::CLI_SNAPSHOT?;
  let addr = std::env::var("DENO_UNSTABLE_CONTROL_SOCK").ok()?;

  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    std::env::remove_var("DENO_UNSTABLE_CONTROL_SOCK")
  };

  let argv0 = args[0].clone();

  Some(async move {
    use tokio::{
      io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
      net::{TcpListener, UnixSocket},
    };
    #[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
    use tokio_vsock::VsockAddr;
    #[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
    use tokio_vsock::VsockListener;

    init_v8(&Flags::default());

    let unconfigured = deno_runtime::UnconfiguredRuntime::new::<
      deno_resolver::npm::DenoInNpmPackageChecker,
      crate::npm::CliNpmResolver,
      crate::sys::CliSys,
    >(deno_runtime::UnconfiguredRuntimeOptions {
      startup_snapshot,
      create_params: deno_lib::worker::create_isolate_create_params(&crate::sys::CliSys::default()),
      shared_array_buffer_store: Some(roots.shared_array_buffer_store.clone()),
      compiled_wasm_module_store: Some(roots.compiled_wasm_module_store.clone()),
      additional_extensions: vec![],
    });

    let (rx, mut tx): (
      Box<dyn AsyncRead + Unpin>,
      Box<dyn AsyncWrite + Send + Unpin>,
    ) = match addr.split_once(':') {
      Some(("tcp", addr)) => {
        let listener = TcpListener::bind(addr).await?;
        let (stream, _) = listener.accept().await?;
        let (rx, tx) = stream.into_split();
        (Box::new(rx), Box::new(tx))
      }
      Some(("unix", path)) => {
        let socket = UnixSocket::new_stream()?;
        socket.bind(path)?;
        let listener = socket.listen(1)?;
        let (stream, _) = listener.accept().await?;
        let (rx, tx) = stream.into_split();
        (Box::new(rx), Box::new(tx))
      }
      #[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
      Some(("vsock", addr)) => {
        let Some((cid, port)) = addr.split_once(':') else {
          deno_core::anyhow::bail!("invalid vsock addr");
        };
        let cid = if cid == "-1" { u32::MAX } else { cid.parse()? };
        let port = port.parse()?;
        let addr = VsockAddr::new(cid, port);
        let listener = VsockListener::bind(addr)?;
        let (stream, _) = listener.accept().await?;
        let (rx, tx) = stream.into_split();
        (Box::new(rx), Box::new(tx))
      }
      _ => {
        deno_core::anyhow::bail!("invalid control sock");
      }
    };

    let mut buf = Vec::with_capacity(1024);
    BufReader::new(rx).read_until(b'\n', &mut buf).await?;

    tokio::spawn(async move {
      deno_runtime::deno_http::SERVE_NOTIFIER.notified().await;

      #[derive(deno_core::serde::Serialize)]
      enum Event {
        Serving,
      }

      let mut buf = deno_core::serde_json::to_vec(&Event::Serving).unwrap();
      buf.push(b'\n');
      let _ = tx.write_all(&buf).await;
    });

    #[derive(deno_core::serde::Deserialize)]
    struct Start {
      cwd: String,
      args: Vec<String>,
      env: Vec<(String, String)>,
    }

    let cmd: Start = deno_core::serde_json::from_slice(&buf)?;

    std::env::set_current_dir(cmd.cwd)?;

    for (k, v) in cmd.env {
      // SAFETY: We're doing this before any threads are created.
      unsafe { std::env::set_var(k, v) };
    }

    let args = [argv0]
      .into_iter()
      .chain(cmd.args.into_iter().map(Into::into))
      .collect();

    Ok(Some((unconfigured, args)))
  })
}

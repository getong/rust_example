use std::{
  rc::Rc,
  sync::{Arc, Mutex},
};

use deno_core::{error::AnyError, url::Url};
use deno_resolver::npm::{DenoInNpmPackageChecker, NpmResolver};
use deno_runtime::{
  BootstrapOptions, WorkerExecutionMode,
  ops::bootstrap::SnapshotOptions,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};
use tokio::{
  io::{AsyncBufReadExt, BufReader},
  sync::mpsc,
};

use crate::{
  duplex::{DuplexChannelPair, duplex_extension, rust_duplex_driver},
  embed::{EmbedResult, embed_extension},
  module_loader::DirectModuleLoader,
  runtime_paths::{bootstrap_script_path, resolve_target_specifier},
};

mod duplex;
mod embed;
mod module_loader;
mod runtime_paths;

deno_core::extension!(
  snapshot_options_extension,
  options = {
    snapshot_options: SnapshotOptions,
  },
  state = |state, options| {
    state.put::<SnapshotOptions>(options.snapshot_options);
  },
);

#[derive(Debug, Clone)]
struct RuntimeLaunchArgs {
  target_arg: String,
  ts_args: Vec<String>,
  preload_modules: Vec<String>,
  mfa_values: Vec<String>,
  persistent: bool,
}

impl Default for RuntimeLaunchArgs {
  fn default() -> Self {
    Self {
      target_arg: "embed_deno/simple_main.ts".to_string(),
      ts_args: Vec::new(),
      preload_modules: Vec::new(),
      mfa_values: Vec::new(),
      persistent: true,
    }
  }
}

fn parse_runtime_launch_args() -> Result<RuntimeLaunchArgs, AnyError> {
  let mut parsed = RuntimeLaunchArgs::default();
  let mut args = std::env::args().skip(1);
  let mut target_set = false;
  let mut passthrough_mode = false;

  while let Some(arg) = args.next() {
    if passthrough_mode {
      parsed.ts_args.push(arg);
      continue;
    }

    if arg == "--" {
      passthrough_mode = true;
      continue;
    }

    if let Some(value) = arg.strip_prefix("--target=") {
      if value.is_empty() {
        return Err(AnyError::msg("--target= requires a non-empty value"));
      }
      parsed.target_arg = value.to_string();
      target_set = true;
      continue;
    }

    if let Some(value) = arg.strip_prefix("--module=") {
      if value.is_empty() {
        return Err(AnyError::msg("--module= requires a non-empty value"));
      }
      parsed.preload_modules.push(value.to_string());
      continue;
    }

    if let Some(value) = arg.strip_prefix("--mfa=") {
      if value.is_empty() {
        return Err(AnyError::msg("--mfa= requires a non-empty value"));
      }
      parsed.mfa_values.push(value.to_string());
      continue;
    }

    match arg.as_str() {
      "--target" => {
        let value = args
          .next()
          .ok_or_else(|| AnyError::msg("--target requires a value"))?;
        parsed.target_arg = value;
        target_set = true;
      }
      "--module" => {
        let value = args
          .next()
          .ok_or_else(|| AnyError::msg("--module requires a value"))?;
        parsed.preload_modules.push(value);
      }
      "--mfa" => {
        let value = args
          .next()
          .ok_or_else(|| AnyError::msg("--mfa requires a value"))?;
        parsed.mfa_values.push(value);
      }
      "--persistent" => {
        parsed.persistent = true;
      }
      "--oneshot" => {
        parsed.persistent = false;
      }
      _ => {
        if !target_set && !arg.starts_with('-') {
          parsed.target_arg = arg;
          target_set = true;
        } else {
          parsed.ts_args.push(arg);
          parsed.ts_args.extend(args);
          break;
        }
      }
    }
  }

  Ok(parsed)
}

async fn bridge_stdin_messages(tx: mpsc::Sender<String>) -> Result<(), AnyError> {
  let stdin = tokio::io::stdin();
  let mut lines = BufReader::new(stdin).lines();

  loop {
    let maybe_line = lines
      .next_line()
      .await
      .map_err(|err| AnyError::msg(format!("failed to read stdin line: {err}")))?;
    let Some(line) = maybe_line else {
      break;
    };
    let line = line.trim();
    if line.is_empty() {
      continue;
    }
    if tx.send(line.to_string()).await.is_err() {
      break;
    }
  }

  Ok(())
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
  let (result_tx, result_rx) = tokio::sync::oneshot::channel::<Result<(), AnyError>>();

  std::thread::Builder::new()
    .name("libmainworker_duplex_runtime".to_string())
    .spawn(move || {
      let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
      {
        Ok(runtime) => runtime,
        Err(err) => {
          let _ = result_tx.send(Err(AnyError::msg(format!(
            "failed to create current-thread runtime: {err}"
          ))));
          return;
        }
      };

      let local = tokio::task::LocalSet::new();
      let result = local.block_on(&runtime, run_inner());
      let _ = result_tx.send(result);
    })
    .map_err(|err| AnyError::msg(format!("failed to spawn runtime thread: {err}")))?;

  result_rx
    .await
    .map_err(|err| AnyError::msg(format!("runtime thread dropped result: {err}")))?
}

async fn run_inner() -> Result<(), AnyError> {
  // Required by rustls 0.23+ when TLS-backed APIs (for example fetch) are used.
  let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    std::env::set_var("DENO_FORCE_OP_REGISTRATION", "1");
  }

  let launch_args = parse_runtime_launch_args()?;
  let target_specifier = resolve_target_specifier(&launch_args.target_arg)?;
  let preload_modules = launch_args
    .preload_modules
    .iter()
    .map(|specifier| resolve_target_specifier(specifier))
    .collect::<Result<Vec<_>, _>>()?;
  let bootstrap_path = bootstrap_script_path()?;
  let runtime_config_json = serde_json::json!({
    "targetSpecifier": target_specifier.clone(),
    "targetArg": launch_args.target_arg.clone(),
    "args": launch_args.ts_args.clone(),
    "modules": preload_modules.clone(),
    "mfa": launch_args.mfa_values.clone(),
  });

  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    std::env::set_var("LIBMAINWORKER_TARGET_SPECIFIER", &target_specifier);
    std::env::set_var(
      "LIBMAINWORKER_RUNTIME_CONFIG",
      serde_json::to_string(&runtime_config_json)
        .map_err(|err| AnyError::msg(format!("failed to serialize runtime config: {err}")))?,
    );
  }

  println!("target script: {}", launch_args.target_arg);
  println!("target specifier: {target_specifier}");
  println!("typescript args: {:?}", launch_args.ts_args);
  println!("preload modules: {:?}", preload_modules);
  println!("mfa values: {:?}", launch_args.mfa_values);
  println!("persistent mode: {}", launch_args.persistent);

  let (rust_to_ts_tx, rust_to_ts_rx) = mpsc::channel::<String>(64);
  let (ts_to_rust_tx, ts_to_rust_rx) = mpsc::channel::<String>(64);
  let (process_msg_tx, process_msg_rx) = mpsc::channel::<String>(256);
  let embed_result = Arc::new(Mutex::new(EmbedResult::default()));
  let embed_result_for_worker = embed_result.clone();

  let root_permissions = deno_runtime::deno_permissions::PermissionsContainer::allow_all(Arc::new(
    deno_runtime::permissions::RuntimePermissionDescriptorParser::new(
      sys_traits::impls::RealSys::default(),
    ),
  ));
  let module_loader = Rc::new(DirectModuleLoader::new());

  let main_module = Url::from_file_path(&bootstrap_path)
    .map(deno_core::ModuleSpecifier::from)
    .map_err(|_| {
      AnyError::msg(format!(
        "failed to convert bootstrap path to file url: {}",
        bootstrap_path.display()
      ))
    })?;
  let services = WorkerServiceOptions::<
    DenoInNpmPackageChecker,
    NpmResolver<sys_traits::impls::RealSys>,
    sys_traits::impls::RealSys,
  > {
    deno_rt_native_addon_loader: None,
    module_loader,
    permissions: root_permissions,
    blob_store: Arc::new(deno_runtime::deno_web::BlobStore::default()),
    broadcast_channel: Default::default(),
    feature_checker: Arc::new(deno_runtime::FeatureChecker::default()),
    node_services: None,
    npm_process_state_provider: None,
    root_cert_store_provider: None,
    fetch_dns_resolver: Default::default(),
    shared_array_buffer_store: None,
    compiled_wasm_module_store: None,
    v8_code_cache: None,
    bundle_provider: None,
    fs: Arc::new(deno_runtime::deno_fs::RealFs),
  };
  let options = WorkerOptions {
    startup_snapshot: deno_snapshots::CLI_SNAPSHOT,
    bootstrap: BootstrapOptions {
      mode: WorkerExecutionMode::Run,
      enable_testing_features: true,
      args: launch_args.ts_args.clone(),
      ..Default::default()
    },
    extensions: vec![
      snapshot_options_extension::init(SnapshotOptions::default()),
      duplex_extension(DuplexChannelPair {
        inbound_rx: rust_to_ts_rx,
        outbound_tx: ts_to_rust_tx,
      }),
      embed_extension(embed_result_for_worker),
    ],
    ..Default::default()
  };
  let mut worker = MainWorker::bootstrap_from_options(&main_module.clone(), services, options);

  println!("mainworker created with direct MainWorker bootstrap + duplex extension");
  let worker_main_module = main_module.clone();
  let mut worker_task = tokio::task::spawn_local(async move {
    let module_id = worker.preload_main_module(&worker_main_module).await?;
    worker.evaluate_module(module_id).await?;
    worker.run_event_loop(false).await?;
    Ok::<(), AnyError>(())
  });
  let mut driver_task = tokio::task::spawn_local(rust_duplex_driver(
    rust_to_ts_tx,
    ts_to_rust_rx,
    process_msg_rx,
    launch_args.persistent,
  ));
  let mut stdin_bridge_task = tokio::task::spawn_local(async move {
    if let Err(err) = bridge_stdin_messages(process_msg_tx).await {
      eprintln!("[rust] stdin bridge failed: {err}");
    }
  });
  let mut worker_completed = false;
  let mut driver_completed = false;
  let mut stdin_bridge_completed = false;

  loop {
    tokio::select! {
      worker_result = &mut worker_task, if !worker_completed => {
        worker_result
          .map_err(|err| AnyError::msg(format!("mainworker task join error: {err}")))??;
        worker_completed = true;
        if driver_completed && (stdin_bridge_completed || stdin_bridge_task.is_finished()) {
          break;
        }
      }
      driver_result = &mut driver_task, if !driver_completed => {
        driver_result
          .map_err(|err| AnyError::msg(format!("duplex driver task join error: {err}")))??;
        driver_completed = true;
        if worker_completed && (stdin_bridge_completed || stdin_bridge_task.is_finished()) {
          break;
        }
      }
      _ = &mut stdin_bridge_task, if !stdin_bridge_completed => {
        stdin_bridge_completed = true;
        if worker_completed && driver_completed {
          break;
        }
      }
    }

    if worker_completed && driver_completed {
      break;
    }
  }

  if !stdin_bridge_completed {
    stdin_bridge_task.abort();
  }

  if !worker_completed {
    return Err(AnyError::msg("worker did not complete"));
  }
  if !driver_completed {
    return Err(AnyError::msg("rust duplex driver did not complete"));
  }

  println!("worker completed (direct MainWorker mode, no CLI exit code)");
  println!("rust <-> ts channel communication completed");

  if let Ok(mut guard) = embed_result.lock() {
    if !guard.exit_data_printed {
      if let Some(json) = guard.exit_data.take() {
        println!("EMBED_DENO_EXIT_DATA={json}");
      }
    } else {
      let _ = guard.exit_data.take();
    }
    if !guard.result_printed {
      if let Some(json) = guard.result.take() {
        println!("EMBED_DENO_RESULT={json}");
      }
    } else {
      let _ = guard.result.take();
    }
    if guard.exit_data_printed {
      guard.exit_data_printed = false;
    }
    if guard.result_printed {
      guard.result_printed = false;
    }
  }

  Ok(())
}

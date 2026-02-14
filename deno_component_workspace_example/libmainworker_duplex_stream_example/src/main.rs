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
use tokio::sync::mpsc;

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

  let target_arg = std::env::args()
    .nth(1)
    .unwrap_or_else(|| "embed_deno/simple_main.ts".to_string());
  let target_specifier = resolve_target_specifier(&target_arg)?;
  let bootstrap_path = bootstrap_script_path()?;

  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    std::env::set_var("LIBMAINWORKER_TARGET_SPECIFIER", &target_specifier);
  }

  println!("target script: {target_arg}");
  println!("target specifier: {target_specifier}");

  let (rust_to_ts_tx, rust_to_ts_rx) = mpsc::channel::<String>(64);
  let (ts_to_rust_tx, ts_to_rust_rx) = mpsc::channel::<String>(64);
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
  let mut worker_future = std::pin::pin!(async move {
    let module_id = worker.preload_main_module(&worker_main_module).await?;
    worker.evaluate_module(module_id).await?;
    worker.run_event_loop(false).await?;
    Ok::<(), AnyError>(())
  });
  let mut driver_future = std::pin::pin!(rust_duplex_driver(rust_to_ts_tx, ts_to_rust_rx));
  let mut worker_completed = false;
  let mut driver_completed = false;

  loop {
    tokio::select! {
      worker_result = &mut worker_future, if !worker_completed => {
        worker_result?;
        worker_completed = true;
        if driver_completed {
          break;
        }
      }
      driver_result = &mut driver_future, if !driver_completed => {
        driver_result?;
        driver_completed = true;
        if worker_completed {
          break;
        }
      }
    }
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
    if let Some(json) = guard.exit_data.take() {
      println!("EMBED_DENO_EXIT_DATA={json}");
    }
    if let Some(json) = guard.result.take() {
      println!("EMBED_DENO_RESULT={json}");
    }
  }

  Ok(())
}

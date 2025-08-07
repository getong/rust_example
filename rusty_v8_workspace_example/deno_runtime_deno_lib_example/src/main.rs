use std::{rc::Rc, sync::Arc};

use deno_resolver::npm::{DenoInNpmPackageChecker, NpmResolver};
use deno_runtime::{
  deno_core::{ModuleSpecifier, error::AnyError},
  deno_fs::RealFs,
  deno_permissions::PermissionsContainer,
  ops::bootstrap::SnapshotOptions,
  permissions::RuntimePermissionDescriptorParser,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};
use sys_traits::impls::RealSys;

mod module_loader;
mod npm_cache;
mod npm_downloader;
mod npm_registry;
mod npm_specifier;

use module_loader::CustomModuleLoader;

// Extension to provide SnapshotOptions
deno_runtime::deno_core::extension!(
    snapshot_options_extension,
    options = {
        snapshot_options: SnapshotOptions,
    },
    state = |state, options| {
        state.put::<SnapshotOptions>(options.snapshot_options);
    },
);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), AnyError> {
  println!("🚀 Executing TypeScript with npm imports using MainWorker");
  println!("=======================================================");

  // Parse command line arguments
  let args: Vec<String> = std::env::args().collect();
  if args.len() != 4 {
    eprintln!("Usage: {} <api_key> <api_secret> <user_id>", args[0]);
    std::process::exit(1);
  }

  let api_key = &args[1];
  let api_secret = &args[2];
  let user_id = &args[3];

  let current_dir = std::env::current_dir()?;
  let ts_path = current_dir.join("src/example.ts");
  let main_module = ModuleSpecifier::from_file_path(&ts_path).unwrap();

  println!("📁 Loading: {}", main_module);

  // Create MainWorker with our custom module loader
  let mut worker = create_main_worker(&main_module).await?;

  // Load Node.js compatibility layer first
  let node_compat_path = current_dir.join("src/node_compat.js");
  let node_compat_code = std::fs::read_to_string(&node_compat_path)?;
  worker.execute_script("node_compat", node_compat_code.into())?;

  // Inject the arguments as global variables
  let setup_script = format!(
    r#"
    globalThis.api_key = "{}";
    globalThis.api_secret = "{}";
    globalThis.user_id = "{}";
    "#,
    api_key, api_secret, user_id
  );

  worker.execute_script("setup_globals", setup_script.into())?;

  println!("🔄 Executing module...");
  println!("📋 Output from TypeScript execution:");
  println!("{}", "=".repeat(50));

  // Execute the module
  worker.execute_main_module(&main_module).await?;

  // Run the event loop to completion
  worker.run_event_loop(false).await?;

  println!("{}", "=".repeat(50));
  println!("✅ Execution completed!");

  Ok(())
}

async fn create_main_worker(main_module: &ModuleSpecifier) -> Result<MainWorker, AnyError> {
  let fs = Arc::new(RealFs);
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(RealSys));
  let permissions = PermissionsContainer::allow_all(permission_desc_parser);

  // Create our custom module loader
  let module_loader = CustomModuleLoader::new(fs.clone());

  // Set up worker service options with our npm-capable module loader
  let services = WorkerServiceOptions::<DenoInNpmPackageChecker, NpmResolver<RealSys>, RealSys> {
    module_loader: Rc::new(module_loader),
    permissions,
    blob_store: Default::default(),
    broadcast_channel: Default::default(),
    feature_checker: Default::default(),
    fs: fs.clone(),
    node_services: Default::default(),
    npm_process_state_provider: Default::default(),
    root_cert_store_provider: Default::default(),
    fetch_dns_resolver: Default::default(),
    shared_array_buffer_store: Default::default(),
    compiled_wasm_module_store: Default::default(),
    v8_code_cache: Default::default(),
    deno_rt_native_addon_loader: Default::default(),
  };

  // Set up worker options with our extension
  let snapshot_options = SnapshotOptions::default();
  let options = WorkerOptions {
    extensions: vec![snapshot_options_extension::init(snapshot_options)],
    ..Default::default()
  };

  // Create the MainWorker
  Ok(MainWorker::bootstrap_from_options(
    main_module,
    services,
    options,
  ))
}

// Clean up unused functions - focusing on real execution

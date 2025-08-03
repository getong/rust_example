use std::{rc::Rc, sync::Arc};

use deno_core::{FsModuleLoader, ModuleSpecifier, error::AnyError, op2};
use deno_fs::RealFs;
use deno_resolver::npm::{ByonmInNpmPackageChecker, ByonmNpmResolver};
use deno_runtime::{
  deno_permissions::PermissionsContainer,
  ops::bootstrap::SnapshotOptions,
  permissions::RuntimePermissionDescriptorParser,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};
use sys_traits::impls::RealSys;

// Create a minimal op to ensure the extension is properly initialized
#[op2(fast)]
fn op_hello(#[string] name: &str) {
  println!("Hello, {}!", name);
}

// Create an extension with the op
deno_core::extension!(example_ext, ops = [op_hello]);

// Extension to provide SnapshotOptions
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
  // Use simple.js instead of the TypeScript file with HTTP imports
  let js_path = std::env::current_dir()?.join("tests/simple.js");
  // let js_path = std::env::current_dir()?.join("tests/http_import.ts");
  let main_module = ModuleSpecifier::from_file_path(&js_path).unwrap();

  let fs = Arc::new(RealFs);
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(RealSys));

  // Set up worker service options
  let services =
    WorkerServiceOptions::<ByonmInNpmPackageChecker, ByonmNpmResolver<RealSys>, RealSys> {
      module_loader: Rc::new(FsModuleLoader),
      permissions: PermissionsContainer::allow_all(permission_desc_parser),
      blob_store: Default::default(),
      broadcast_channel: Default::default(),
      feature_checker: Default::default(),
      fs: fs.clone(),
      node_services: None,
      npm_process_state_provider: Default::default(),
      root_cert_store_provider: Default::default(),
      fetch_dns_resolver: Default::default(),
      shared_array_buffer_store: Default::default(),
      compiled_wasm_module_store: Default::default(),
      v8_code_cache: Default::default(),
      deno_rt_native_addon_loader: Default::default(),
    };

  let snapshot_options = SnapshotOptions::default();

  // Set up worker options with our extension
  let options = WorkerOptions {
    extensions: vec![
      snapshot_options_extension::init(snapshot_options),
      example_ext::init(),
    ],
    ..Default::default()
  };

  // Bootstrap the worker
  let mut worker = MainWorker::bootstrap_from_options(&main_module, services, options);

  // Execute the main module
  worker.execute_main_module(&main_module).await?;

  // Run the event loop
  worker.run_event_loop(false).await?;

  Ok(())
}

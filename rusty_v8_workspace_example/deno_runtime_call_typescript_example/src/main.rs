use std::{rc::Rc, sync::Arc};

use deno_core::{FsModuleLoader, error::AnyError};
use deno_fs::RealFs;
use deno_resolver::npm::{DenoInNpmPackageChecker, NpmResolver};
use deno_runtime::{
  BootstrapOptions,
  deno_permissions::PermissionsContainer,
  permissions::RuntimePermissionDescriptorParser,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};

#[tokio::main]
async fn main() -> Result<(), AnyError> {
  let js_path = std::env::current_dir()?.join("tests/http_import.ts");
  let main_module = deno_core::resolve_path(&js_path.to_string_lossy(), &std::env::current_dir()?)?;

  let create_web_worker_cb = std::sync::Arc::new(|_| {
    panic!("Web workers are not supported");
  });

  let options = WorkerOptions {
    bootstrap: BootstrapOptions {
      args: vec![],
      cpu_count: 1,
      enable_testing_features: false,
      locale: "en".to_string(),
      location: None,
      user_agent: "deno_runtime_example".to_string(),
      inspect: false,
      ..Default::default()
    },
    extensions: vec![],
    startup_snapshot: None,
    unsafely_ignore_certificate_errors: None,
    seed: None,
    create_web_worker_cb,
    format_js_error_fn: None,
    ..Default::default()
  };

  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(
    sys_traits::impls::RealSys,
  ));

  let services = WorkerServiceOptions::<
    DenoInNpmPackageChecker,
    NpmResolver<sys_traits::impls::RealSys>,
    sys_traits::impls::RealSys,
  > {
    module_loader: Rc::new(FsModuleLoader),
    permissions: PermissionsContainer::allow_all(permission_desc_parser),
    blob_store: Default::default(),
    broadcast_channel: Default::default(),
    feature_checker: Default::default(),
    fs: Arc::new(RealFs),
    node_services: Default::default(),
    npm_process_state_provider: Default::default(),
    root_cert_store_provider: Default::default(),
    fetch_dns_resolver: Default::default(),
    shared_array_buffer_store: Default::default(),
    compiled_wasm_module_store: Default::default(),
    v8_code_cache: Default::default(),
    deno_rt_native_addon_loader: None,
  };

  let mut worker = MainWorker::bootstrap_from_options(&main_module, services, options);

  worker.execute_main_module(&main_module).await?;
  worker.run_event_loop(false).await?;

  Ok(())
}

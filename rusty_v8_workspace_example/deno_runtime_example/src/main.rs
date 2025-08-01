use std::{path::Path, rc::Rc, sync::Arc};

use deno_core::{FsModuleLoader, ModuleSpecifier, error::AnyError, op2};
use deno_fs::RealFs;
use deno_resolver::npm::{DenoInNpmPackageChecker, NpmResolver};
use deno_runtime::{
  deno_permissions::PermissionsContainer,
  permissions::RuntimePermissionDescriptorParser,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};

#[op2(fast)]
fn op_hello(#[string] text: &str) {
  println!("Hello {} from an op!", text);
}

deno_core::extension!(
    hello_runtime,
    ops = [op_hello],
    esm_entry_point = "ext:hello_runtime/bootstrap.js",
    esm = [dir "src", "bootstrap.js"]
);

#[tokio::main]
async fn main() -> Result<(), AnyError> {
  let js_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.js");
  let main_module = ModuleSpecifier::from_file_path(js_path).unwrap();
  eprintln!("Running {main_module}...");
  let fs = Arc::new(RealFs);
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(
    sys_traits::impls::RealSys,
  ));
  let mut worker = MainWorker::bootstrap_from_options(
    &main_module,
    WorkerServiceOptions::<
      DenoInNpmPackageChecker,
      NpmResolver<sys_traits::impls::RealSys>,
      sys_traits::impls::RealSys,
    > {
      deno_rt_native_addon_loader: None,
      module_loader: Rc::new(FsModuleLoader),
      permissions: PermissionsContainer::allow_all(permission_desc_parser),
      blob_store: Default::default(),
      broadcast_channel: Default::default(),
      feature_checker: Default::default(),
      node_services: Default::default(),
      npm_process_state_provider: Default::default(),
      root_cert_store_provider: Default::default(),
      fetch_dns_resolver: Default::default(),
      shared_array_buffer_store: Default::default(),
      compiled_wasm_module_store: Default::default(),
      v8_code_cache: Default::default(),
      fs,
    },
    WorkerOptions {
      extensions: vec![hello_runtime::init()],
      ..Default::default()
    },
  );
  worker.execute_main_module(&main_module).await?;
  worker.run_event_loop(false).await?;
  Ok(())
}

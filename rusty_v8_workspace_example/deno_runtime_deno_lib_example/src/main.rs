use std::{rc::Rc, sync::Arc};

use deno_resolver::npm::{DenoInNpmPackageChecker, NpmResolver};
use deno_runtime::{
  deno_core::{FsModuleLoader, url::Url},
  deno_fs::RealFs,
  deno_permissions::PermissionsContainer,
  permissions::RuntimePermissionDescriptorParser,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};

pub fn prepare_worker(main_module: &Url) -> MainWorker {
  let fs = Arc::new(RealFs);
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(
    sys_traits::impls::RealSys,
  ));
  let permissions = PermissionsContainer::allow_all(permission_desc_parser);

  let worker_service_options = WorkerServiceOptions::<
    DenoInNpmPackageChecker,
    NpmResolver<sys_traits::impls::RealSys>,
    sys_traits::impls::RealSys,
  > {
    deno_rt_native_addon_loader: Default::default(),
    module_loader: Rc::new(FsModuleLoader),
    permissions: permissions,
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
  };

  let options = WorkerOptions::default();

  return MainWorker::bootstrap_from_options(main_module, worker_service_options, options);
}

// main function
#[tokio::main]
async fn main() {
  let current_dir = std::env::current_dir().unwrap();
  let main_module = deno_runtime::deno_core::resolve_path("./src/example.ts", &current_dir);

  let main_module = match main_module {
    Ok(module) => module,
    Err(err) => {
      eprintln!("Error resolving main module path: {}", err);
      return;
    }
  };

  let mut prepared_worker = prepare_worker(&main_module);

  if let Err(e) = prepared_worker.execute_main_module(&main_module).await {
    eprintln!("Error executing main module: {}", e);
    return;
  }

  if let Err(e) = prepared_worker.run_event_loop(false).await {
    eprintln!("Error running event loop: {}", e);
    return;
  }
}

#[cfg(test)]
mod tests {

  use deno_runtime::deno_core::resolve_path;

  use super::*;

  #[tokio::test]
  async fn it_works() {
    let current_dir = std::env::current_dir().unwrap();
    let main_module = resolve_path("./examples/hello.js", &current_dir);

    let main_module = match main_module {
      Ok(module) => module,
      Err(err) => {
        eprintln!("Error resolving main module path: {}", err);
        return;
      }
    };

    let mut prepared_worker = prepare_worker(&main_module);

    if let Err(e) = prepared_worker.execute_main_module(&main_module).await {
      eprintln!("Error executing main module: {}", e);
      panic!("Error executing main module: {}", e);
    }

    if let Err(e) = prepared_worker.run_event_loop(false).await {
      eprintln!("Error running event loop: {}", e);
      panic!("Error running event loop: {}", e);
    }
  }
}

// copy from https://github.com/denoland/deno/issues/29174
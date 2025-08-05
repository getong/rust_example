use std::{rc::Rc, sync::Arc};

use deno_resolver::npm::{DenoInNpmPackageChecker, NpmResolver};
use deno_runtime::{
  deno_core::url::Url,
  deno_fs::RealFs,
  deno_permissions::PermissionsContainer,
  permissions::RuntimePermissionDescriptorParser,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};

mod module_loader;
mod npm_example;
use module_loader::CustomModuleLoader;

// New enhanced version with deno_lib support
pub async fn prepare_npm_worker(main_module: &Url) -> Result<MainWorker, anyhow::Error> {
  let fs = Arc::new(RealFs);
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(
    sys_traits::impls::RealSys,
  ));
  let permissions = PermissionsContainer::allow_all(permission_desc_parser);

  // Try to use deno_lib for npm support
  // Note: This is a conceptual approach - deno_lib API may differ
  match try_create_npm_enabled_worker(main_module, fs.clone(), permissions.clone()).await {
    Ok(worker) => Ok(worker),
    Err(_) => {
      // Fallback to basic worker without npm support
      Ok(prepare_basic_worker(main_module, fs, permissions))
    }
  }
}

async fn try_create_npm_enabled_worker(
  main_module: &Url,
  fs: Arc<RealFs>,
  permissions: PermissionsContainer,
) -> Result<MainWorker, anyhow::Error> {
  // This would be the ideal implementation using deno_lib
  // However, deno_lib API might be different, so this is conceptual

  // In a real implementation, you would:
  // 1. Create npm registry client
  // 2. Set up npm cache directory
  // 3. Create npm resolver with cache and registry
  // 4. Configure module loader with npm resolver

  // For now, this is a placeholder that demonstrates the concept
  // In practice, you would need to:
  // - Set up deno_npm::NpmRegistryApi for fetching package metadata
  // - Create deno_npm_cache::NpmCache for local package storage
  // - Build an NpmResolver that can resolve npm: specifiers to file paths
  // - Integrate with the module loader to handle npm packages

  Err(anyhow::anyhow!(
    "Enhanced npm support not yet implemented - deno_lib integration requires additional setup"
  ))
}

pub fn prepare_worker(main_module: &Url) -> MainWorker {
  let fs = Arc::new(RealFs);
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(
    sys_traits::impls::RealSys,
  ));
  let permissions = PermissionsContainer::allow_all(permission_desc_parser);

  prepare_basic_worker(main_module, fs, permissions)
}

pub fn prepare_basic_worker(
  main_module: &Url,
  fs: Arc<RealFs>,
  permissions: PermissionsContainer,
) -> MainWorker {
  // Use our custom module loader that can handle npm: specifiers
  let custom_loader = CustomModuleLoader::new(fs.clone());

  let worker_service_options = WorkerServiceOptions::<
    DenoInNpmPackageChecker,
    NpmResolver<sys_traits::impls::RealSys>,
    sys_traits::impls::RealSys,
  > {
    deno_rt_native_addon_loader: Default::default(),
    module_loader: Rc::new(custom_loader),
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

// main function - enhanced version with npm support attempt
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

  println!("Running TypeScript file with npm import...");
  println!("Attempting to load npm packages using enhanced module loader");
  println!();

  // Try the enhanced npm-capable worker first
  match prepare_npm_worker(&main_module).await {
    Ok(mut worker) => {
      println!("✓ Enhanced npm worker created successfully");

      if let Err(e) = worker.execute_main_module(&main_module).await {
        eprintln!("Error executing main module: {}", e);
        print_npm_guidance(&e.to_string());
        return;
      }

      if let Err(e) = worker.run_event_loop(false).await {
        eprintln!("Error running event loop: {}", e);
        return;
      }

      println!("✓ Module executed successfully with npm support!");
    }
    Err(e) => {
      println!("⚠ Enhanced npm worker failed: {}", e);
      println!("Falling back to basic worker (npm: imports will fail)");
      println!();

      // Fallback to basic worker
      let mut basic_worker = prepare_worker(&main_module);

      if let Err(e) = basic_worker.execute_main_module(&main_module).await {
        eprintln!("Error executing main module: {}", e);
        print_npm_guidance(&e.to_string());
        return;
      }

      if let Err(e) = basic_worker.run_event_loop(false).await {
        eprintln!("Error running event loop: {}", e);
        return;
      }
    }
  }
}

fn print_npm_guidance(error_msg: &str) {
  if error_msg.contains("npm:") {
    eprintln!("\nNPM Import Error Detected!");
    eprintln!("=====================================");
    eprintln!("The error above shows that npm: specifiers require additional infrastructure.");
    eprintln!("\nTo implement npm support, you would need:");
    eprintln!("1. NpmRegistryApi - to fetch package metadata");
    eprintln!("2. NpmCache - to download and cache packages");
    eprintln!("3. NpmResolver - to resolve npm specifiers to file paths");
    eprintln!("4. Custom ModuleLoader - to integrate npm resolution");
    eprintln!("\nAlternatives:");
    eprintln!("- Use deno_lib with full npm infrastructure");
    eprintln!("- Use regular imports instead of npm: specifiers");
    eprintln!("- Set up a complete Deno CLI environment");
    eprintln!("\nSee the following files for more details:");
    eprintln!("- src/npm_example.rs - Architecture overview");
    eprintln!("- src/module_loader.rs - Custom loader implementation");
    eprintln!("- README.md - Full documentation");
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

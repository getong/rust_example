use std::{path::PathBuf, rc::Rc, sync::Arc};

use anyhow::Result;
use deno_runtime::{
  BootstrapOptions, WorkerExecutionMode,
  deno_broadcast_channel::InMemoryBroadcastChannel,
  deno_core::{
    ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
    RequestedModuleType, ResolutionKind, resolve_import,
  },
  deno_fs::RealFs,
  deno_io::Stdio,
  deno_permissions::{Permissions, PermissionsContainer},
  deno_tls::rustls::crypto::{CryptoProvider, ring},
  permissions::RuntimePermissionDescriptorParser,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};
use node_resolver::{
  InNpmPackageChecker, NpmPackageFolderResolver, UrlOrPathRef, errors::PackageFolderResolveError,
};
use sys_traits::impls::RealSys;
use url::Url;

#[derive(Clone)]
struct NoopModuleLoader;

#[derive(Clone)]
struct NoopInNpmPackageChecker;

#[derive(Clone)]
struct NoopNpmPackageFolderResolver;

impl InNpmPackageChecker for NoopInNpmPackageChecker {
  fn in_npm_package(&self, _specifier: &Url) -> bool {
    false
  }
}

impl NpmPackageFolderResolver for NoopNpmPackageFolderResolver {
  fn resolve_package_folder_from_package(
    &self,
    _name: &str,
    _referrer: &UrlOrPathRef,
  ) -> Result<PathBuf, PackageFolderResolveError> {
    // For a noop resolver, we'll just return a dummy path
    // In a real implementation, this would resolve npm packages
    Ok(PathBuf::from("/dev/null"))
  }
}

impl ModuleLoader for NoopModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, deno_error::JsErrorBox> {
    resolve_import(specifier, referrer)
      .map_err(|e| deno_error::JsErrorBox::new("TypeError", e.to_string()))
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleSpecifier>,
    _is_dyn_import: bool,
    _requested_module_type: RequestedModuleType,
  ) -> ModuleLoadResponse {
    let module_specifier = module_specifier.clone();

    // Handle node: imports
    if module_specifier.scheme() == "node" {
      // These are built-in modules, they should be handled by deno_node extension
      return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
        ModuleType::JavaScript,
        // Return empty source, the actual implementation is in the extension
        ModuleSourceCode::String("".to_string().into()).into(),
        &module_specifier,
        None,
      )));
    }

    // For file:// URLs, load from disk
    if module_specifier.scheme() == "file" {
      let path = module_specifier.to_file_path().unwrap();
      let code = std::fs::read_to_string(&path).unwrap();

      let module_type = if path.extension().and_then(|s| s.to_str()) == Some("ts") {
        ModuleType::JavaScript // In a real implementation, we'd transpile TypeScript
      } else {
        ModuleType::JavaScript
      };

      return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
        module_type,
        ModuleSourceCode::String(code.into()).into(),
        &module_specifier,
        None,
      )));
    }

    ModuleLoadResponse::Sync(Err(deno_error::JsErrorBox::new(
      "TypeError",
      format!("Unsupported module specifier: {}", module_specifier),
    )))
  }
}

fn main() -> Result<()> {
  // Install the default crypto provider for rustls (required for HTTPS)
  CryptoProvider::install_default(ring::default_provider())
    .expect("Failed to install default crypto provider");

  // Create a current thread runtime as Deno expects
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()?;

  runtime.block_on(async {
    env_logger::init();

    // Create a temporary directory for our DENO_DIR
    let temp_dir = std::env::temp_dir().join("deno_node_runtime_example");
    std::fs::create_dir_all(&temp_dir)?;

    // Set up stdio
    let stdio = Stdio {
      stdin: deno_runtime::deno_io::StdioPipe::inherit(),
      stdout: deno_runtime::deno_io::StdioPipe::inherit(),
      stderr: deno_runtime::deno_io::StdioPipe::inherit(),
    };

    // Create the main module specifier
    let script_path = std::env::current_dir()?.join("test_https.ts");
    let script_url = Url::from_file_path(&script_path).unwrap();
    let main_module = ModuleSpecifier::from(script_url);

    // Create worker options
    let options = WorkerOptions {
      bootstrap: BootstrapOptions {
        deno_version: "0.1.0".to_string(),
        args: vec![],
        cpu_count: std::thread::available_parallelism()
          .map(|p| p.get())
          .unwrap_or(1),
        log_level: deno_runtime::WorkerLogLevel::Info,
        enable_op_summary_metrics: false,
        enable_testing_features: true,
        locale: "en-US".to_string(),
        location: None,
        color_level: deno_terminal::colors::ColorLevel::Ansi256,
        unstable_features: vec![],
        user_agent: "deno_node_runtime_example".to_string(),
        inspect: false,
        is_standalone: false,
        has_node_modules_dir: false,
        argv0: None,
        node_debug: None,
        node_ipc_fd: None,
        mode: WorkerExecutionMode::Run,
        no_legacy_abort: false,
        serve_port: None,
        serve_host: None,
        auto_serve: false,
        otel_config: Default::default(),
        close_on_idle: false,
      },
      extensions: vec![],
      startup_snapshot: None,
      create_params: None,
      unsafely_ignore_certificate_errors: None,
      seed: None,
      create_web_worker_cb: Arc::new(|_| {
        unreachable!("Web workers are not supported in this example")
      }),
      format_js_error_fn: None,
      maybe_inspector_server: None,
      should_break_on_first_statement: false,
      should_wait_for_inspector_session: false,
      strace_ops: None,
      cache_storage_dir: None,
      origin_storage_dir: None,
      stdio,
      skip_op_registration: false,
      enable_raw_imports: false,
      enable_stack_trace_arg_in_ops: false,
      unconfigured_runtime: None,
    };

    // Create permissions (allow all for this example)
    let sys = RealSys;
    let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(sys));
    let permissions = PermissionsContainer::new(permission_desc_parser, Permissions::allow_all());

    // Create service options
    let fs = Arc::new(RealFs);
    let services = WorkerServiceOptions {
      deno_rt_native_addon_loader: None,
      module_loader: Rc::new(NoopModuleLoader),
      permissions,
      blob_store: Default::default(),
      broadcast_channel: InMemoryBroadcastChannel::default(),
      feature_checker: Default::default(),
      node_services: None,
      npm_process_state_provider: None,
      root_cert_store_provider: None,
      fetch_dns_resolver: Default::default(),
      shared_array_buffer_store: None,
      compiled_wasm_module_store: None,
      v8_code_cache: None,
      fs,
    };

    // Create the main worker with proper generic type parameters
    let mut worker = MainWorker::bootstrap_from_options::<
      NoopInNpmPackageChecker,
      NoopNpmPackageFolderResolver,
      RealSys,
    >(&main_module.clone(), services, options);

    println!("Deno runtime initialized successfully!");

    // First, let's preload the Node.js modules we need
    println!("Preloading node:http and node:https modules...");

    let http_specifier = ModuleSpecifier::parse("node:http")?;
    let https_specifier = ModuleSpecifier::parse("node:https")?;

    // Preload node:http
    let http_id = worker.preload_main_module(&http_specifier).await?;
    println!("Preloaded node:http module with ID: {}", http_id);

    // Preload node:https
    let https_id = worker.preload_main_module(&https_specifier).await?;
    println!("Preloaded node:https module with ID: {}", https_id);

    // Now create and execute our TypeScript script
    println!("\nExecuting TypeScript script: {}", script_path.display());

    // Load and evaluate the main module
    let id = worker.preload_main_module(&main_module).await?;
    worker.evaluate_module(id).await?;

    // Run the event loop
    worker.run_event_loop(false).await?;

    println!("\nScript execution completed successfully!");

    Ok(())
  })
}

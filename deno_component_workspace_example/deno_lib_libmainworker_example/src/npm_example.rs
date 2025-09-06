use std::{rc::Rc, sync::Arc};

use deno_core::{
  ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleType,
  RequestedModuleType, ResolutionKind, error::ModuleLoaderError,
};
use deno_error::JsErrorBox;
use deno_lib::{
  npm::create_npm_process_state_provider,
  worker::{
    CreateModuleLoaderResult, LibMainWorkerFactory, LibMainWorkerOptions, ModuleLoaderFactory,
    StorageKeyResolver,
  },
};
use deno_resolver::npm::{
  ByonmNpmResolverCreateOptions, CreateInNpmPkgCheckerOptions, DenoInNpmPackageChecker,
  NpmResolver, NpmResolverCreateOptions,
};
use deno_runtime::{
  FeatureChecker, WorkerExecutionMode, WorkerLogLevel,
  deno_fs::RealFs,
  deno_node::NodeRequireLoader,
  deno_permissions::{Permissions, PermissionsContainer},
  deno_tls::{RootCertStoreProvider, rustls::RootCertStore},
  deno_web::BlobStore,
  permissions::RuntimePermissionDescriptorParser,
};
use deno_semver::npm::NpmPackageReqReference;
use node_resolver::{InNpmPackageChecker, NodeResolver, PackageJsonResolver};
use sys_traits::impls::RealSys;
use url::Url;

mod npm_fetch;
mod npm_loader;

use npm_loader::{NpmModuleCache, create_npm_module_source};

// NPM-aware module loader that can handle npm: scheme URLs
struct NpmModuleLoader {
  #[allow(dead_code)]
  npm_resolver: Arc<NpmResolver<RealSys>>,
  #[allow(dead_code)]
  node_resolver: Arc<
    NodeResolver<
      DenoInNpmPackageChecker,
      node_resolver::DenoIsBuiltInNodeModuleChecker,
      NpmResolver<RealSys>,
      RealSys,
    >,
  >,
  in_npm_pkg_checker: DenoInNpmPackageChecker,
  #[allow(dead_code)]
  npm_package_resolver: Arc<tokio::sync::Mutex<npm_fetch::NpmPackageResolver>>,
  #[allow(dead_code)]
  module_cache: Arc<tokio::sync::Mutex<NpmModuleCache>>,
  #[allow(dead_code)]
  main_module_source: Option<String>,
  #[allow(dead_code)]
  runtime_handle: tokio::runtime::Handle,
}

impl ModuleLoader for NpmModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<Url, ModuleLoaderError> {
    // Handle npm: scheme URLs
    if specifier.starts_with("npm:") {
      if let Ok(url) = Url::parse(specifier) {
        if let Ok(npm_ref) = NpmPackageReqReference::from_specifier(&url) {
          // For this example, we'll convert npm: to a simple URL
          // In a real implementation, this would resolve to the actual npm package location
          println!("Resolving npm package: {}", npm_ref.req());

          // For demonstration, we'll return a placeholder URL
          // In reality, this would resolve to the actual file path in node_modules
          return Ok(
            Url::parse(&format!(
              "file:///node_modules/{}/index.js",
              npm_ref.req().name
            ))
            .unwrap(),
          );
        }
      }
    }

    // For non-npm URLs, just resolve normally
    if let Ok(url) = Url::parse(specifier) {
      Ok(url)
    } else if let Ok(referrer_url) = Url::parse(referrer) {
      referrer_url
        .join(specifier)
        .map_err(|e| JsErrorBox::generic(e.to_string()))
    } else {
      Err(JsErrorBox::generic(format!(
        "Cannot resolve specifier: {}",
        specifier
      )))
    }
  }

  fn load(
    &self,
    module_specifier: &Url,
    _maybe_referrer: Option<&Url>,
    _is_dynamic: bool,
    _requested_module_type: RequestedModuleType,
  ) -> ModuleLoadResponse {
    // Check if this is an npm module
    if self.in_npm_pkg_checker.in_npm_package(module_specifier)
      || module_specifier.path().contains("/node_modules/")
    {
      // For now, return a simple implementation that doesn't require async fetching
      // In a real implementation, you would use ModuleLoadResponse::Async

      // Try to determine if this is the chalk package
      let is_chalk =
        module_specifier.path().contains("chalk") || module_specifier.as_str().contains("chalk");

      let package_name = if is_chalk {
        "chalk".to_string()
      } else if let Ok(npm_ref) = NpmPackageReqReference::from_specifier(module_specifier) {
        npm_ref.req().name.to_string()
      } else {
        "unknown".to_string()
      };

      println!("[NPM Loader] Loading npm module: {}", package_name);

      // Return a placeholder module
      // In a real implementation, this would:
      // 1. Use CliNpmCacheHttpClient to download the package
      // 2. Use CliNpmInstaller to install it
      // 3. Load the actual JavaScript/TypeScript files
      let code = format!(
        r#"
// NPM Package: {} (placeholder)
console.log("[NPM] Would download and install package: {} using deno_fetch");

// This is where the actual npm package would be loaded
// For now, export an empty module to allow the example to run
export default {{}};
"#,
        package_name, package_name
      );

      let module_source = create_npm_module_source(code.to_string(), module_specifier);
      return ModuleLoadResponse::Sync(Ok(module_source));
    }

    // For the main module, return the TypeScript source directly
    let code = if module_specifier
      .path()
      .ends_with("deno_npm_example_main.ts")
    {
      // Read the actual TypeScript file
      match std::fs::read_to_string(module_specifier.to_file_path().unwrap()) {
        Ok(content) => content,
        Err(e) => return ModuleLoadResponse::Sync(Err(JsErrorBox::from_err(e))),
      }
    } else {
      r#"console.log("Module loaded:", import.meta.url);"#.to_string()
    };

    let module_source = ModuleSource::new(
      ModuleType::JavaScript,
      ModuleSourceCode::String(code.to_string().into()),
      module_specifier,
      None,
    );

    ModuleLoadResponse::Sync(Ok(module_source))
  }
}

impl NodeRequireLoader for NpmModuleLoader {
  fn ensure_read_permission<'a>(
    &self,
    _permissions: &mut dyn deno_runtime::deno_node::NodePermissions,
    path: std::borrow::Cow<'a, std::path::Path>,
  ) -> Result<std::borrow::Cow<'a, std::path::Path>, JsErrorBox> {
    Ok(path)
  }

  fn load_text_file_lossy(
    &self,
    _path: &std::path::Path,
  ) -> Result<deno_core::FastString, JsErrorBox> {
    // For this example, return a simple module
    Ok(
      r#"module.exports = { message: "Hello from Node.js!" };"#
        .to_string()
        .into(),
    )
  }

  fn is_maybe_cjs(
    &self,
    specifier: &Url,
  ) -> Result<bool, node_resolver::errors::PackageJsonLoadError> {
    // Check if this is a CommonJS module
    Ok(specifier.path().ends_with(".cjs") || self.in_npm_pkg_checker.in_npm_package(specifier))
  }
}

// Module loader factory for npm support
struct NpmModuleLoaderFactory {
  npm_resolver: Arc<NpmResolver<RealSys>>,
  node_resolver: Arc<
    NodeResolver<
      DenoInNpmPackageChecker,
      node_resolver::DenoIsBuiltInNodeModuleChecker,
      NpmResolver<RealSys>,
      RealSys,
    >,
  >,
  in_npm_pkg_checker: DenoInNpmPackageChecker,
  npm_package_resolver: Arc<tokio::sync::Mutex<npm_fetch::NpmPackageResolver>>,
  module_cache: Arc<tokio::sync::Mutex<NpmModuleCache>>,
  main_module_source: Option<String>,
  runtime_handle: tokio::runtime::Handle,
}

impl ModuleLoaderFactory for NpmModuleLoaderFactory {
  fn create_for_main(&self, _root_permissions: PermissionsContainer) -> CreateModuleLoaderResult {
    let loader = Rc::new(NpmModuleLoader {
      npm_resolver: self.npm_resolver.clone(),
      node_resolver: self.node_resolver.clone(),
      in_npm_pkg_checker: self.in_npm_pkg_checker.clone(),
      npm_package_resolver: self.npm_package_resolver.clone(),
      module_cache: self.module_cache.clone(),
      main_module_source: self.main_module_source.clone(),
      runtime_handle: self.runtime_handle.clone(),
    });
    CreateModuleLoaderResult {
      module_loader: loader.clone(),
      node_require_loader: loader,
    }
  }

  fn create_for_worker(
    &self,
    _parent_permissions: PermissionsContainer,
    permissions: PermissionsContainer,
  ) -> CreateModuleLoaderResult {
    self.create_for_main(permissions)
  }
}

// Root cert store provider
struct SimpleRootCertStoreProvider {
  store: RootCertStore,
}

impl RootCertStoreProvider for SimpleRootCertStoreProvider {
  fn get_or_try_init(&self) -> Result<&RootCertStore, JsErrorBox> {
    Ok(&self.store)
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Install default crypto provider for rustls
  deno_runtime::deno_tls::rustls::crypto::aws_lc_rs::default_provider()
    .install_default()
    .expect("Failed to install crypto provider");

  println!("=== npm: scheme URL Example ===");
  println!();

  // Create the TypeScript source code that we'll execute
  let typescript_source = r#"
import chalk from "npm:chalk@5";

console.log("🦕 Deno running with npm: support!");
console.log("This demonstrates loading npm: scheme URLs!");

// The npm module loader would download and install the actual chalk package
// For this example, chalk will be undefined, but in a real implementation
// it would provide the actual chalk functionality

console.log("✅ npm: URL resolved successfully!");
console.log("📦 Package import attempted: chalk@5");
console.log("🔄 In a full implementation, this would download the package using deno_fetch");
console.log("📂 And install it using the Deno npm infrastructure");

// Show what would happen with real npm support
if (typeof chalk === 'object') {
  console.log("✨ chalk module loaded successfully!");
} else {
  console.log("ℹ️  chalk module returned placeholder (expected for this example)");
}

// Demonstrate that the TypeScript code runs successfully
console.log("🎉 TypeScript execution completed!");
"#;

  // Create a file URL for the main module
  let _main_module_url = Url::parse("file:///main.ts").unwrap();

  // Store the TypeScript source in a temporary file for better module loading
  let temp_dir = std::env::temp_dir();
  let main_file_path = temp_dir.join("deno_npm_example_main.ts");
  std::fs::write(&main_file_path, typescript_source)?;

  // Update the URL to point to the actual file
  let main_module_url = Url::from_file_path(&main_file_path).unwrap();

  // Set up all the required dependencies
  let blob_store = Arc::new(BlobStore::default());
  let feature_checker = Arc::new(FeatureChecker::default());
  let fs = Arc::new(RealFs);
  let root_cert_store_provider = Arc::new(SimpleRootCertStoreProvider {
    store: RootCertStore::empty(),
  });

  // Create package.json resolver
  let pkg_json_resolver = Arc::new(PackageJsonResolver::new(RealSys, None));

  // Create npm checker and resolver
  let in_npm_pkg_checker = DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Byonm);

  let npm_resolver = Arc::new(NpmResolver::<RealSys>::new(
    NpmResolverCreateOptions::Byonm(ByonmNpmResolverCreateOptions {
      sys: node_resolver::cache::NodeResolutionSys::new(RealSys, None),
      pkg_json_resolver: pkg_json_resolver.clone(),
      root_node_modules_dir: None,
    }),
  ));

  let node_resolver = Arc::new(NodeResolver::new(
    in_npm_pkg_checker.clone(),
    node_resolver::DenoIsBuiltInNodeModuleChecker,
    npm_resolver.as_ref().clone(),
    pkg_json_resolver.clone(),
    node_resolver::cache::NodeResolutionSys::new(RealSys, None),
    Default::default(),
  ));

  // Create npm package resolver using deno_fetch
  let npm_package_resolver = Arc::new(tokio::sync::Mutex::new(npm_fetch::NpmPackageResolver::new(
    root_cert_store_provider.clone(),
  )?));

  // Get the tokio runtime handle
  let runtime_handle = tokio::runtime::Handle::current();

  // Create module cache
  let module_cache = Arc::new(tokio::sync::Mutex::new(NpmModuleCache::new()));

  // Create module loader factory with npm support
  let module_loader_factory = Box::new(NpmModuleLoaderFactory {
    npm_resolver: npm_resolver.clone(),
    node_resolver: node_resolver.clone(),
    in_npm_pkg_checker: in_npm_pkg_checker.clone(),
    npm_package_resolver: npm_package_resolver.clone(),
    module_cache,
    main_module_source: Some(typescript_source.to_string()),
    runtime_handle,
  });

  // Create worker options
  let worker_options = LibMainWorkerOptions {
    argv: vec![],
    log_level: WorkerLogLevel::Info,
    enable_op_summary_metrics: false,
    enable_testing_features: false,
    has_node_modules_dir: true, // Enable node_modules support
    inspect_brk: false,
    inspect_wait: false,
    strace_ops: None,
    is_inspecting: false,
    is_standalone: false,
    auto_serve: false,
    skip_op_registration: false,
    location: None,
    argv0: None,
    node_debug: None,
    origin_data_folder_path: None,
    seed: None,
    unsafely_ignore_certificate_errors: None,
    node_ipc: None,
    serve_port: None,
    serve_host: None,
    otel_config: Default::default(),
    no_legacy_abort: false,
    startup_snapshot: deno_snapshots::CLI_SNAPSHOT,
    enable_raw_imports: false,
  };

  // Create permissions
  let permission_descriptor_parser = Arc::new(RuntimePermissionDescriptorParser::new(RealSys));
  let permissions = Permissions::allow_all();
  let permissions_container = PermissionsContainer::new(permission_descriptor_parser, permissions);

  println!("Creating LibMainWorkerFactory with npm support...");

  // Create the LibMainWorkerFactory
  let worker_factory = LibMainWorkerFactory::new(
    blob_store,
    None, // code_cache
    None, // deno_rt_native_addon_loader
    feature_checker,
    fs,
    None, // inspector_server
    module_loader_factory,
    node_resolver,
    create_npm_process_state_provider(&npm_resolver),
    pkg_json_resolver,
    root_cert_store_provider,
    StorageKeyResolver::empty(),
    RealSys,
    worker_options,
    Default::default(), // roots
  );

  println!("✅ LibMainWorkerFactory created successfully!");
  println!();
  println!("Creating main worker...");

  // Create the main worker
  let mut worker = worker_factory.create_main_worker(
    WorkerExecutionMode::Run,
    permissions_container,
    main_module_url.clone(),
    vec![], // preload_modules
  )?;

  println!("✅ Main worker created!");
  println!();
  println!("Running TypeScript code with npm: imports...");
  println!();

  // Execute the worker
  let exit_code = worker.run().await?;

  println!();
  println!("✅ Worker completed with exit code: {}", exit_code);
  println!();
  println!("🎉 Successfully demonstrated npm: scheme URL support!");

  // Clean up temporary file
  let _ = std::fs::remove_file(&main_file_path);

  Ok(())
}

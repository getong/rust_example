use std::{rc::Rc, sync::Arc};

mod npm_helpers;

use deno_core::{
  ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleType,
  error::ModuleLoaderError,
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
use node_resolver::{NodeResolver, PackageJsonResolver};
use sys_traits::impls::RealSys;
use url::Url;

// Simple module loader that can load the main module
#[derive(Debug)]
struct SimpleModuleLoader {
  main_module_content: String,
  main_module_url: Url,
}

impl ModuleLoader for SimpleModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    _referrer: &str,
    _kind: deno_core::ResolutionKind,
  ) -> Result<Url, ModuleLoaderError> {
    if specifier == self.main_module_url.as_str() || specifier == "file:///example.js" {
      Ok(self.main_module_url.clone())
    } else {
      Err(JsErrorBox::generic(format!(
        "Module not found: {}",
        specifier
      )))
    }
  }

  fn load(
    &self,
    module_specifier: &Url,
    _maybe_referrer: Option<&Url>,
    _is_dynamic: bool,
    _requested_module_type: deno_core::RequestedModuleType,
  ) -> ModuleLoadResponse {
    if module_specifier == &self.main_module_url {
      let module_source = ModuleSource::new(
        ModuleType::JavaScript,
        ModuleSourceCode::String(self.main_module_content.clone().into()),
        module_specifier,
        None,
      );
      ModuleLoadResponse::Sync(Ok(module_source))
    } else {
      ModuleLoadResponse::Sync(Err(JsErrorBox::type_error(format!(
        "Module not found: {}",
        module_specifier
      ))))
    }
  }
}

impl NodeRequireLoader for SimpleModuleLoader {
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
    Err(JsErrorBox::type_error(
      "File loading not implemented in simple loader",
    ))
  }

  fn is_maybe_cjs(
    &self,
    _specifier: &Url,
  ) -> Result<bool, node_resolver::errors::PackageJsonLoadError> {
    Ok(false)
  }
}

// Simple module loader factory
struct SimpleModuleLoaderFactory {
  main_module_content: String,
  main_module_url: Url,
}

impl ModuleLoaderFactory for SimpleModuleLoaderFactory {
  fn create_for_main(&self, _root_permissions: PermissionsContainer) -> CreateModuleLoaderResult {
    let loader = Rc::new(SimpleModuleLoader {
      main_module_content: self.main_module_content.clone(),
      main_module_url: self.main_module_url.clone(),
    });
    CreateModuleLoaderResult {
      module_loader: loader.clone(),
      node_require_loader: loader,
    }
  }

  fn create_for_worker(
    &self,
    _parent_permissions: PermissionsContainer,
    _permissions: PermissionsContainer,
  ) -> CreateModuleLoaderResult {
    self.create_for_main(_permissions)
  }
}

// Simple root cert store provider
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
  // Create JavaScript content to execute
  let js_content = r#"
console.log("🦕 Hello from Deno LibMainWorker!");
console.log("This code is running inside a worker created by create_main_worker()!");
console.log("Deno version:", Deno.version);
"#;

  // Create a file URL for the main module
  let main_module_url = Url::parse("file:///example.js").unwrap();

  // Set up all the required dependencies
  let blob_store = Arc::new(BlobStore::default());
  let feature_checker = Arc::new(FeatureChecker::default());
  let fs = Arc::new(RealFs);
  let root_cert_store_provider = Arc::new(SimpleRootCertStoreProvider {
    store: RootCertStore::empty(),
  });

  // Create module loader factory
  let module_loader_factory = Box::new(SimpleModuleLoaderFactory {
    main_module_content: js_content.to_string(),
    main_module_url: main_module_url.clone(),
  });

  // Create package.json resolver
  let pkg_json_resolver = Arc::new(PackageJsonResolver::new(RealSys, None));

  // Create npm checker and resolver using Byonm (simpler approach)
  let in_npm_pkg_checker = DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Byonm);

  let npm_resolver = NpmResolver::<RealSys>::new(NpmResolverCreateOptions::Byonm(
    ByonmNpmResolverCreateOptions {
      sys: node_resolver::cache::NodeResolutionSys::new(RealSys, None),
      pkg_json_resolver: pkg_json_resolver.clone(),
      root_node_modules_dir: None,
    },
  ));

  let node_resolver = Arc::new(NodeResolver::new(
    in_npm_pkg_checker,
    node_resolver::DenoIsBuiltInNodeModuleChecker,
    npm_resolver.clone(),
    pkg_json_resolver.clone(),
    node_resolver::cache::NodeResolutionSys::new(RealSys, None),
    Default::default(),
  ));

  // Create worker options
  let worker_options = LibMainWorkerOptions {
    argv: vec![],
    log_level: WorkerLogLevel::Info,
    enable_op_summary_metrics: false,
    enable_testing_features: false,
    has_node_modules_dir: false,
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

  // Create the LibMainWorkerFactory (based on cli/rt/run.rs)
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

  // THIS IS THE ACTUAL CALL TO create_main_worker()!
  let mut worker = worker_factory.create_main_worker(
    WorkerExecutionMode::Run,
    permissions_container,
    main_module_url,
    vec![], // preload_modules
  )?;

  println!("✅ create_main_worker() called successfully!");
  println!("✅ LibMainWorker instance created!");
  println!();
  println!("3️⃣  Running the worker...");

  // Execute the worker
  let exit_code = worker.run().await?;

  println!();
  println!("✅ Worker completed with exit code: {}", exit_code);
  println!();
  println!("🎉 Successfully demonstrated LibMainWorkerFactory.create_main_worker()!");

  Ok(())
}

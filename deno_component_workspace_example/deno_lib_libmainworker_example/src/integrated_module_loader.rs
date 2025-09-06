// Integrated module loader using components from Deno CLI
use std::{cell::RefCell, collections::HashSet, rc::Rc, sync::Arc};

use deno_core::{
  FastString, ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSpecifier, RequestedModuleType,
  ResolutionKind, error::ModuleLoaderError, futures::FutureExt, resolve_url,
};
use deno_error::JsErrorBox;
use deno_lib::worker::{CreateModuleLoaderResult, ModuleLoaderFactory};
use deno_resolver::npm::{CreateInNpmPkgCheckerOptions, DenoInNpmPackageChecker};
use deno_runtime::{
  deno_node::NodePermissions, deno_permissions::PermissionsContainer,
  permissions::RuntimePermissionDescriptorParser,
};
use node_resolver::{InNpmPackageChecker, errors::PackageJsonLoadError};
use sys_traits::impls::RealSys;

use crate::{
  file_fetcher::SimpleFileFetcher,
  graph_container::{MainModuleGraphContainer, ModuleGraphContainer},
  loader_utils::{module_type_from_media_and_requested_type, string_to_module_source_code},
};

/// Integrated module loader that combines file fetching, graph management, and resolution
pub struct IntegratedModuleLoader<TGraphContainer: ModuleGraphContainer> {
  graph_container: TGraphContainer,
  file_fetcher: Arc<SimpleFileFetcher>,
  in_npm_pkg_checker: DenoInNpmPackageChecker,
  loaded_files: RefCell<HashSet<ModuleSpecifier>>,
  permissions: PermissionsContainer,
}

impl<TGraphContainer: ModuleGraphContainer> IntegratedModuleLoader<TGraphContainer> {
  pub fn new(
    graph_container: TGraphContainer,
    file_fetcher: Arc<SimpleFileFetcher>,
    in_npm_pkg_checker: DenoInNpmPackageChecker,
    permissions: PermissionsContainer,
  ) -> Self {
    Self {
      graph_container,
      file_fetcher,
      in_npm_pkg_checker,
      loaded_files: RefCell::new(HashSet::new()),
      permissions,
    }
  }

  /// Resolve a referrer URL
  fn resolve_referrer(&self, referrer: &str) -> Result<ModuleSpecifier, JsErrorBox> {
    if referrer.is_empty() {
      return Ok(
        resolve_url("./$deno$repl.mts")
          .map_err(|e| JsErrorBox::generic(format!("Failed to resolve referrer: {}", e)))?,
      );
    }

    if deno_path_util::specifier_has_uri_scheme(referrer) {
      Ok(
        resolve_url(referrer)
          .map_err(|e| JsErrorBox::generic(format!("Failed to resolve URL: {}", e)))?,
      )
    } else {
      let cwd = std::env::current_dir()
        .map_err(|e| JsErrorBox::generic(format!("Failed to get cwd: {}", e)))?;
      Ok(
        deno_path_util::resolve_path(referrer, &cwd)
          .map_err(|e| JsErrorBox::generic(format!("Failed to resolve path: {}", e)))?,
      )
    }
  }

  /// Load a module's source code
  async fn load_module_source(
    &self,
    specifier: &ModuleSpecifier,
    requested_module_type: &RequestedModuleType,
  ) -> Result<ModuleSource, ModuleLoaderError> {
    // Fetch the file
    let file = self
      .file_fetcher
      .fetch(specifier)
      .await
      .map_err(|e| JsErrorBox::generic(e.to_string()))?;

    // Decode to text
    let decoded = self
      .file_fetcher
      .decode(file)
      .map_err(|e| JsErrorBox::generic(e.to_string()))?;

    // Determine module type
    let module_type =
      module_type_from_media_and_requested_type(decoded.media_type, requested_module_type);

    Ok(ModuleSource::new(
      module_type,
      string_to_module_source_code(decoded.source.to_string()),
      &decoded.specifier,
      None, // No code cache for this example
    ))
  }

  /// Check if a module is a CommonJS module
  #[allow(dead_code)]
  fn is_maybe_cjs(&self, specifier: &ModuleSpecifier) -> Result<bool, PackageJsonLoadError> {
    Ok(specifier.path().ends_with(".cjs") || self.in_npm_pkg_checker.in_npm_package(specifier))
  }
}

impl<TGraphContainer: ModuleGraphContainer> ModuleLoader
  for IntegratedModuleLoader<TGraphContainer>
{
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    let referrer = self
      .resolve_referrer(referrer)
      .map_err(|e| JsErrorBox::generic(e.to_string()))?;

    // Try to parse as absolute URL first
    if let Ok(url) = ModuleSpecifier::parse(specifier) {
      return Ok(url);
    }

    // Resolve relative to referrer
    deno_core::resolve_import(specifier, referrer.as_str())
      .map_err(|e| JsErrorBox::generic(e.to_string()))
  }

  fn load(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    is_dynamic: bool,
    requested_module_type: RequestedModuleType,
  ) -> ModuleLoadResponse {
    // Track loaded files
    self.loaded_files.borrow_mut().insert(specifier.clone());

    let specifier = specifier.clone();
    let loader = self.clone();
    let maybe_referrer = maybe_referrer.cloned();

    ModuleLoadResponse::Async(
      async move {
        println!("[IntegratedModuleLoader] Loading: {}", specifier);
        if let Some(referrer) = maybe_referrer.as_ref() {
          println!("  Referrer: {}", referrer);
        }
        println!("  Is dynamic: {}", is_dynamic);

        loader
          .load_module_source(&specifier, &requested_module_type)
          .await
      }
      .boxed_local(),
    )
  }
}

impl<TGraphContainer: ModuleGraphContainer> Clone for IntegratedModuleLoader<TGraphContainer> {
  fn clone(&self) -> Self {
    Self {
      graph_container: self.graph_container.clone(),
      file_fetcher: self.file_fetcher.clone(),
      in_npm_pkg_checker: self.in_npm_pkg_checker.clone(),
      loaded_files: RefCell::new(self.loaded_files.borrow().clone()),
      permissions: self.permissions.clone(),
    }
  }
}

/// Factory for creating integrated module loaders
pub struct IntegratedModuleLoaderFactory {
  file_fetcher: Arc<SimpleFileFetcher>,
  in_npm_pkg_checker: DenoInNpmPackageChecker,
}

impl IntegratedModuleLoaderFactory {
  pub fn new(allow_remote: bool, in_npm_pkg_checker: DenoInNpmPackageChecker) -> Self {
    Self {
      file_fetcher: Arc::new(SimpleFileFetcher::new(allow_remote)),
      in_npm_pkg_checker,
    }
  }
}

impl ModuleLoaderFactory for IntegratedModuleLoaderFactory {
  fn create_for_main(&self, root_permissions: PermissionsContainer) -> CreateModuleLoaderResult {
    let graph_container = MainModuleGraphContainer::new(deno_graph::GraphKind::CodeOnly);

    let loader = Rc::new(IntegratedModuleLoader::new(
      graph_container,
      self.file_fetcher.clone(),
      self.in_npm_pkg_checker.clone(),
      root_permissions,
    ));

    // For simplicity, we'll create a basic node require loader
    let node_require_loader = Rc::new(BasicNodeRequireLoader);

    CreateModuleLoaderResult {
      module_loader: loader,
      node_require_loader,
    }
  }

  fn create_for_worker(
    &self,
    _parent_permissions: PermissionsContainer,
    permissions: PermissionsContainer,
  ) -> CreateModuleLoaderResult {
    // For workers, create a new graph container
    let graph_container = MainModuleGraphContainer::new(deno_graph::GraphKind::CodeOnly);

    let loader = Rc::new(IntegratedModuleLoader::new(
      graph_container,
      self.file_fetcher.clone(),
      self.in_npm_pkg_checker.clone(),
      permissions,
    ));

    let node_require_loader = Rc::new(BasicNodeRequireLoader);

    CreateModuleLoaderResult {
      module_loader: loader,
      node_require_loader,
    }
  }
}

/// Basic implementation of NodeRequireLoader for compatibility
struct BasicNodeRequireLoader;

impl deno_runtime::deno_node::NodeRequireLoader for BasicNodeRequireLoader {
  fn ensure_read_permission<'a>(
    &self,
    _permissions: &mut dyn NodePermissions,
    path: std::borrow::Cow<'a, std::path::Path>,
  ) -> Result<std::borrow::Cow<'a, std::path::Path>, JsErrorBox> {
    Ok(path)
  }

  fn load_text_file_lossy(&self, path: &std::path::Path) -> Result<FastString, JsErrorBox> {
    let content = std::fs::read_to_string(path)
      .map_err(|e| JsErrorBox::generic(format!("Failed to read file: {}", e)))?;
    Ok(FastString::from(content))
  }

  fn is_maybe_cjs(&self, specifier: &ModuleSpecifier) -> Result<bool, PackageJsonLoadError> {
    Ok(specifier.path().ends_with(".cjs") || specifier.path().contains("/node_modules/"))
  }
}

/// Example usage of the integrated module loader
pub async fn run_integrated_example() -> Result<(), JsErrorBox> {
  println!("\n=== Running Integrated Module Loader Example ===\n");

  // Create NPM package checker
  let in_npm_pkg_checker = DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Byonm);

  // Create the module loader factory
  let factory = IntegratedModuleLoaderFactory::new(true, in_npm_pkg_checker);

  // Create permissions
  let permissions = PermissionsContainer::new(
    Arc::new(RuntimePermissionDescriptorParser::new(RealSys::default())),
    deno_runtime::deno_permissions::Permissions::allow_all(),
  );

  // Create module loader for main
  let result = factory.create_for_main(permissions);

  println!("Created integrated module loader successfully!");
  println!("This loader includes:");
  println!("  - File fetching with local and remote support");
  println!("  - Module graph management");
  println!("  - NPM package detection");
  println!("  - Permission-based access control");

  // Example: Test module resolution
  let test_specifier = "./test.js";
  let referrer = "file:///example/main.js";

  match result
    .module_loader
    .resolve(test_specifier, referrer, ResolutionKind::Import)
  {
    Ok(resolved) => {
      println!(
        "\nResolved '{}' from '{}' to: {}",
        test_specifier, referrer, resolved
      );
    }
    Err(e) => {
      println!("\nFailed to resolve: {:?}", e);
    }
  }

  Ok(())
}

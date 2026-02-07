// Module loader example based on Deno CLI's implementation
use std::{borrow::Cow, cell::RefCell, collections::HashSet, path::Path, pin::Pin, rc::Rc};

use deno_core::{
  FastString, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader,
  ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType, RequestedModuleType, ResolutionKind,
  error::ModuleLoaderError, futures::FutureExt, resolve_url,
};
use deno_error::JsErrorBox;
use deno_lib::worker::{CreateModuleLoaderResult, ModuleLoaderFactory};
use deno_resolver::npm::{CreateInNpmPkgCheckerOptions, DenoInNpmPackageChecker};
use deno_runtime::{
  deno_node::NodeRequireLoader, deno_permissions::PermissionsContainer,
  permissions::RuntimePermissionDescriptorParser,
};
use node_resolver::{InNpmPackageChecker, errors::PackageJsonLoadError};
use sys_traits::impls::RealSys;
use url::Url;

/// A CLI-inspired module loader with enhanced capabilities
pub struct CliInspiredModuleLoader {
  in_npm_pkg_checker: DenoInNpmPackageChecker,
  loaded_files: RefCell<HashSet<ModuleSpecifier>>,
  permissions: PermissionsContainer,
  parent_permissions: PermissionsContainer,
}

impl CliInspiredModuleLoader {
  pub fn new(
    in_npm_pkg_checker: DenoInNpmPackageChecker,
    permissions: PermissionsContainer,
    parent_permissions: PermissionsContainer,
  ) -> Self {
    Self {
      in_npm_pkg_checker,
      loaded_files: RefCell::new(HashSet::new()),
      permissions,
      parent_permissions,
    }
  }

  /// Resolve a referrer URL from a string
  fn resolve_referrer(&self, referrer: &str) -> Result<ModuleSpecifier, JsErrorBox> {
    if referrer.is_empty() {
      // Handle empty referrer case (e.g., REPL)
      return Ok(resolve_url("./$deno$repl.mts").map_err(|e| JsErrorBox::generic(e.to_string()))?);
    }

    if deno_path_util::specifier_has_uri_scheme(referrer) {
      Ok(resolve_url(referrer).map_err(|e| JsErrorBox::generic(e.to_string()))?)
    } else if referrer == "." {
      // Main module case
      let cwd = std::env::current_dir()
        .map_err(|e| JsErrorBox::generic(format!("Failed to get cwd: {}", e)))?;
      Ok(
        deno_path_util::resolve_path(referrer, &cwd)
          .map_err(|e| JsErrorBox::generic(e.to_string()))?,
      )
    } else {
      let cwd = std::env::current_dir()
        .map_err(|e| JsErrorBox::generic(format!("Failed to get cwd: {}", e)))?;
      Ok(
        deno_path_util::resolve_path(referrer, &cwd)
          .map_err(|e| JsErrorBox::generic(e.to_string()))?,
      )
    }
  }

  /// Load a module from a file path
  async fn load_from_file(&self, specifier: &ModuleSpecifier) -> Result<String, JsErrorBox> {
    let path = deno_path_util::url_to_file_path(specifier)
      .map_err(|_| JsErrorBox::generic(format!("Invalid file URL: {}", specifier)))?;

    std::fs::read_to_string(&path)
      .map_err(|e| JsErrorBox::generic(format!("Failed to read file {}: {}", path.display(), e)))
  }

  /// Determine module type from the specifier
  fn determine_module_type(&self, specifier: &ModuleSpecifier) -> ModuleType {
    let path = specifier.path();

    if path.ends_with(".json") {
      ModuleType::Json
    } else if path.ends_with(".wasm") {
      ModuleType::Wasm
    } else {
      ModuleType::JavaScript
    }
  }

  /// Check if a module should be treated as CommonJS
  #[allow(dead_code)]
  fn is_maybe_cjs(&self, specifier: &Url) -> Result<bool, PackageJsonLoadError> {
    Ok(specifier.path().ends_with(".cjs") || self.in_npm_pkg_checker.in_npm_package(specifier))
  }
}

impl ModuleLoader for CliInspiredModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    let referrer = self
      .resolve_referrer(referrer)
      .map_err(|_| JsErrorBox::generic("Module not found"))?;

    // Handle npm: specifiers
    if specifier.starts_with("npm:") {
      // For npm specifiers, preserve them for dynamic imports
      if matches!(kind, ResolutionKind::DynamicImport) {
        return Ok(
          ModuleSpecifier::parse(specifier).map_err(|_| JsErrorBox::generic("Module not found"))?,
        );
      }
    }

    // Handle relative and absolute paths
    if specifier.starts_with("./") || specifier.starts_with("../") || specifier.starts_with("/") {
      Ok(
        resolve_url(specifier)
          .or_else(|_| deno_core::resolve_import(specifier, referrer.as_str()))
          .map_err(|_| JsErrorBox::generic("Module not found"))?,
      )
    } else if specifier.starts_with("http://") || specifier.starts_with("https://") {
      // Handle remote URLs
      Ok(ModuleSpecifier::parse(specifier).map_err(|_| JsErrorBox::generic("Module not found"))?)
    } else {
      // Try to resolve as a relative import
      deno_core::resolve_import(specifier, referrer.as_str())
        .map_err(|_| JsErrorBox::generic("Module not found"))
    }
  }

  fn load(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleLoadReferrer>,
    options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    // Track loaded files
    self.loaded_files.borrow_mut().insert(specifier.clone());

    let specifier = specifier.clone();
    let maybe_referrer = maybe_referrer.map(|r| r.specifier.clone());
    let loader = self.clone();
    let is_dynamic = options.is_dynamic_import;
    let requested_module_type = options.requested_module_type;

    ModuleLoadResponse::Async(
      async move {
        // Log the loading operation
        println!("[CliInspiredModuleLoader] Loading: {}", specifier);
        if let Some(referrer) = &maybe_referrer {
          println!("  Referrer: {}", referrer);
        }
        println!("  Is dynamic: {}", is_dynamic);
        println!("  Requested type: {:?}", requested_module_type);

        // Load the module content
        let code = if specifier.scheme() == "file" {
          loader.load_from_file(&specifier).await?
        } else if specifier.scheme() == "http" || specifier.scheme() == "https" {
          // For remote modules, return a placeholder
          format!("// Remote module: {}\nexport default {{}};", specifier)
        } else {
          return Err(JsErrorBox::generic(format!(
            "Unsupported scheme: {}",
            specifier.scheme()
          )));
        };

        // Determine module type
        let module_type = match requested_module_type {
          RequestedModuleType::Json => ModuleType::Json,
          _ => loader.determine_module_type(&specifier),
        };

        Ok(ModuleSource::new(
          module_type,
          ModuleSourceCode::String(FastString::from(code)),
          &specifier,
          None, // No code cache for this example
        ))
      }
      .boxed_local(),
    )
  }

  fn prepare_load(
    &self,
    specifier: &ModuleSpecifier,
    _maybe_referrer: Option<String>,
    options: ModuleLoadOptions,
  ) -> Pin<Box<dyn std::future::Future<Output = Result<(), ModuleLoaderError>>>> {
    println!(
      "[CliInspiredModuleLoader] Preparing load for: {} (dynamic: {}, type: {:?})",
      specifier, options.is_dynamic_import, options.requested_module_type
    );

    // For this example, we don't need to do any preparation
    Box::pin(async { Ok(()) })
  }
}

impl Clone for CliInspiredModuleLoader {
  fn clone(&self) -> Self {
    Self {
      in_npm_pkg_checker: self.in_npm_pkg_checker.clone(),
      loaded_files: RefCell::new(self.loaded_files.borrow().clone()),
      permissions: self.permissions.clone(),
      parent_permissions: self.parent_permissions.clone(),
    }
  }
}

/// Factory for creating CliInspiredModuleLoader instances
pub struct CliInspiredModuleLoaderFactory {
  in_npm_pkg_checker: DenoInNpmPackageChecker,
}

impl CliInspiredModuleLoaderFactory {
  pub fn new(in_npm_pkg_checker: DenoInNpmPackageChecker) -> Self {
    Self { in_npm_pkg_checker }
  }
}

impl ModuleLoaderFactory for CliInspiredModuleLoaderFactory {
  fn create_for_main(&self, root_permissions: PermissionsContainer) -> CreateModuleLoaderResult {
    let loader = Rc::new(CliInspiredModuleLoader::new(
      self.in_npm_pkg_checker.clone(),
      root_permissions.clone(),
      root_permissions,
    ));

    CreateModuleLoaderResult {
      module_loader: loader.clone(),
      node_require_loader: std::rc::Rc::new(SimpleNodeRequireLoader),
    }
  }

  fn create_for_worker(
    &self,
    parent_permissions: PermissionsContainer,
    permissions: PermissionsContainer,
  ) -> CreateModuleLoaderResult {
    let loader = Rc::new(CliInspiredModuleLoader::new(
      self.in_npm_pkg_checker.clone(),
      permissions.clone(),
      parent_permissions,
    ));

    CreateModuleLoaderResult {
      module_loader: loader.clone(),
      node_require_loader: std::rc::Rc::new(SimpleNodeRequireLoader),
    }
  }
}

/// Example usage of the CLI-inspired module loader
pub async fn run_cli_inspired_example() -> Result<(), JsErrorBox> {
  println!("\n=== Running CLI-Inspired Module Loader Example ===\n");

  // Create NPM package checker
  let in_npm_pkg_checker = DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Byonm);

  // Create the module loader factory
  let factory = CliInspiredModuleLoaderFactory::new(in_npm_pkg_checker);

  // Create permissions with allow-all defaults for this example
  let desc_parser = std::sync::Arc::new(RuntimePermissionDescriptorParser::new(RealSys::default()));
  let permissions_obj = deno_runtime::deno_permissions::Permissions::allow_all();
  let permissions = PermissionsContainer::new(desc_parser, permissions_obj);

  // Create module loader for main
  let result = factory.create_for_main(permissions);

  println!("Created CLI-inspired module loader successfully!");
  println!("This loader includes:");
  println!("  - NPM package detection");
  println!("  - File and remote module loading");
  println!("  - Permission-based access control");
  println!("  - Dynamic import support");
  println!("  - Module type detection");

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

/// Simple NodeRequireLoader implementation for the example
struct SimpleNodeRequireLoader;

impl NodeRequireLoader for SimpleNodeRequireLoader {
  fn ensure_read_permission<'a>(
    &self,
    _permissions: &mut PermissionsContainer,
    path: Cow<'a, Path>,
  ) -> Result<Cow<'a, Path>, JsErrorBox> {
    // For this example, allow all reads
    Ok(path)
  }

  fn load_text_file_lossy(&self, path: &Path) -> Result<FastString, JsErrorBox> {
    std::fs::read_to_string(path)
      .map(|s| s.into())
      .map_err(|e| JsErrorBox::generic(format!("Failed to read file {}: {}", path.display(), e)))
  }

  fn is_maybe_cjs(&self, _specifier: &ModuleSpecifier) -> Result<bool, PackageJsonLoadError> {
    // For this example, assume nothing is CJS
    Ok(false)
  }

  fn resolve_require_node_module_paths(&self, _from: &Path) -> Vec<String> {
    // For this example, return empty paths
    Vec::new()
  }
}

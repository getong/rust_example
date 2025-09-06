// Simple CLI-based module loader
use std::{cell::RefCell, collections::HashSet, rc::Rc, sync::Arc};

use deno_core::{
  FastString, ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier,
  ModuleType, RequestedModuleType, ResolutionKind, error::ModuleLoaderError, futures::FutureExt,
  resolve_url,
};
use deno_error::JsErrorBox;
use deno_lib::worker::{CreateModuleLoaderResult, ModuleLoaderFactory};
use deno_resolver::npm::{CreateInNpmPkgCheckerOptions, DenoInNpmPackageChecker};
use deno_runtime::{
  deno_permissions::{Permissions, PermissionsContainer},
  permissions::RuntimePermissionDescriptorParser,
};
use node_resolver::{InNpmPackageChecker, errors::PackageJsonLoadError};
use sys_traits::impls::RealSys;

/// Simple module loader
pub struct SimpleCliModuleLoader {
  in_npm_pkg_checker: DenoInNpmPackageChecker,
  loaded_files: RefCell<HashSet<ModuleSpecifier>>,
  permissions: PermissionsContainer,
}

impl SimpleCliModuleLoader {
  pub fn new(
    in_npm_pkg_checker: DenoInNpmPackageChecker,
    permissions: PermissionsContainer,
  ) -> Self {
    Self {
      in_npm_pkg_checker,
      loaded_files: RefCell::new(HashSet::new()),
      permissions,
    }
  }

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

  #[allow(dead_code)]
  fn is_maybe_cjs(&self, specifier: &ModuleSpecifier) -> Result<bool, PackageJsonLoadError> {
    Ok(specifier.path().ends_with(".cjs") || self.in_npm_pkg_checker.in_npm_package(specifier))
  }

  fn load_file(&self, specifier: &ModuleSpecifier) -> Result<String, JsErrorBox> {
    if specifier.scheme() == "file" {
      let path = deno_path_util::url_to_file_path(specifier)
        .map_err(|_| JsErrorBox::generic(format!("Invalid file URL: {}", specifier)))?;
      std::fs::read_to_string(&path)
        .map_err(|e| JsErrorBox::generic(format!("Failed to read file {}: {}", path.display(), e)))
    } else {
      Ok(format!(
        "// Mock module: {}\nexport default {{}};",
        specifier
      ))
    }
  }
}

impl ModuleLoader for SimpleCliModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    let referrer = self
      .resolve_referrer(referrer)
      .map_err(|e| JsErrorBox::generic(e.to_string()))?;

    if let Ok(url) = ModuleSpecifier::parse(specifier) {
      return Ok(url);
    }

    deno_core::resolve_import(specifier, referrer.as_str())
      .map_err(|e| JsErrorBox::generic(e.to_string()))
  }

  fn load(
    &self,
    specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleSpecifier>,
    _is_dynamic: bool,
    requested_module_type: RequestedModuleType,
  ) -> ModuleLoadResponse {
    self.loaded_files.borrow_mut().insert(specifier.clone());

    let specifier = specifier.clone();
    let loader = self.clone();

    ModuleLoadResponse::Async(
      async move {
        println!("[SimpleCliModuleLoader] Loading: {}", specifier);

        let code = loader
          .load_file(&specifier)
          .map_err(|e| JsErrorBox::generic(e.to_string()))?;

        let module_type = match requested_module_type {
          RequestedModuleType::Json => ModuleType::Json,
          _ => ModuleType::JavaScript,
        };

        Ok(ModuleSource::new(
          module_type,
          ModuleSourceCode::String(FastString::from(code)),
          &specifier,
          None,
        ))
      }
      .boxed_local(),
    )
  }
}

impl Clone for SimpleCliModuleLoader {
  fn clone(&self) -> Self {
    Self {
      in_npm_pkg_checker: self.in_npm_pkg_checker.clone(),
      loaded_files: RefCell::new(self.loaded_files.borrow().clone()),
      permissions: self.permissions.clone(),
    }
  }
}

/// Factory for simple CLI module loader
pub struct SimpleCliModuleLoaderFactory {
  in_npm_pkg_checker: DenoInNpmPackageChecker,
}

impl SimpleCliModuleLoaderFactory {
  pub fn new() -> Self {
    let in_npm_pkg_checker = DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Byonm);

    Self { in_npm_pkg_checker }
  }
}

impl ModuleLoaderFactory for SimpleCliModuleLoaderFactory {
  fn create_for_main(&self, root_permissions: PermissionsContainer) -> CreateModuleLoaderResult {
    let loader = Rc::new(SimpleCliModuleLoader::new(
      self.in_npm_pkg_checker.clone(),
      root_permissions,
    ));

    let node_require_loader = Rc::new(SimpleNodeRequireLoader);

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
    let loader = Rc::new(SimpleCliModuleLoader::new(
      self.in_npm_pkg_checker.clone(),
      permissions,
    ));

    let node_require_loader = Rc::new(SimpleNodeRequireLoader);

    CreateModuleLoaderResult {
      module_loader: loader,
      node_require_loader,
    }
  }
}

/// Simple NodeRequireLoader implementation
struct SimpleNodeRequireLoader;

impl deno_runtime::deno_node::NodeRequireLoader for SimpleNodeRequireLoader {
  fn ensure_read_permission<'a>(
    &self,
    _permissions: &mut dyn deno_runtime::deno_node::NodePermissions,
    path: std::borrow::Cow<'a, std::path::Path>,
  ) -> Result<std::borrow::Cow<'a, std::path::Path>, JsErrorBox> {
    Ok(path)
  }

  fn load_text_file_lossy(
    &self,
    path: &std::path::Path,
  ) -> Result<deno_core::FastString, JsErrorBox> {
    let content = std::fs::read_to_string(path)
      .map_err(|e| JsErrorBox::generic(format!("Failed to read file: {}", e)))?;
    Ok(deno_core::FastString::from(content))
  }

  fn is_maybe_cjs(&self, specifier: &ModuleSpecifier) -> Result<bool, PackageJsonLoadError> {
    Ok(specifier.path().ends_with(".cjs") || specifier.path().contains("/node_modules/"))
  }
}

/// Example usage
pub async fn run_simple_cli_example() -> Result<(), JsErrorBox> {
  println!("\n=== Running Simple CLI Module Loader Example ===\n");

  let factory = SimpleCliModuleLoaderFactory::new();

  let permissions = PermissionsContainer::new(
    Arc::new(RuntimePermissionDescriptorParser::new(RealSys)),
    Permissions::allow_all(),
  );

  let result = factory.create_for_main(permissions);

  println!("Created simple CLI module loader successfully!");

  let test_specifier = "./test.js";
  let referrer = "file:///example/main.js";

  match result
    .module_loader
    .resolve(test_specifier, referrer, ResolutionKind::Import)
  {
    Ok(resolved) => {
      println!(
        "Resolved '{}' from '{}' to: {}",
        test_specifier, referrer, resolved
      );
    }
    Err(e) => {
      println!("Failed to resolve: {:?}", e);
    }
  }

  Ok(())
}

#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

mod cli;
mod node_imports_resolver;
mod npm_module_loader;

use std::{path::Path, rc::Rc, sync::Arc};

use colored::*;
use deno_ast::{
  EmitOptions, MediaType, ParseParams, SourceMapOption, TranspileModuleOptions, TranspileOptions,
};
use deno_core::{ModuleSpecifier, error::AnyError, op2, serde_json};
use deno_permissions::prompter::{PermissionPrompter, PromptResponse, set_prompter};
use deno_resolver::npm::{
  ByonmNpmResolver, ByonmNpmResolverCreateOptions, CreateInNpmPkgCheckerOptions,
  DenoInNpmPackageChecker,
};
use deno_runtime::{
  deno_core::{
    self, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader, ModuleSource,
    ModuleSourceCode, ModuleType, ResolutionKind, resolve_import,
  },
  deno_fs::RealFs,
  deno_permissions::{Permissions, PermissionsContainer},
  ops::bootstrap::SnapshotOptions,
  permissions::RuntimePermissionDescriptorParser,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};
use deno_semver::npm::NpmPackageReqReference;
use node_resolver::{PackageJsonResolver, cache::NodeResolutionSys};
use sys_traits::impls::RealSys;

#[op2]
#[string]
fn example_custom_op(#[string] text: &str) -> String {
  println!("Hello {} from an op!", text);
  text.to_string() + " from Rust!"
}

deno_runtime::deno_core::extension!(
  example_extension,
  ops = [example_custom_op],
  esm_entry_point = "ext:example_extension/bootstrap.js",
  esm = [dir "src", "bootstrap.js"]
);

deno_runtime::deno_core::extension!(
  snapshot_options_extension,
  options = {
    snapshot_options: SnapshotOptions,
  },
  state = |state, options| {
    state.put::<SnapshotOptions>(options.snapshot_options);
  },
);

struct CustomPrompter;

impl PermissionPrompter for CustomPrompter {
  fn prompt(
    &mut self,
    message: &str,
    name: &str,
    api_name: Option<&str>,
    is_unary: bool,
    _get_stack: Option<deno_permissions::prompter::GetFormattedStackFn>,
  ) -> PromptResponse {
    println!(
      "{}\n{} {}\n{} {}\n{} {:?}\n{} {}",
      "Script is trying to access APIs and needs permission:"
        .yellow()
        .bold(),
      "Message:".bright_blue(),
      message,
      "Name:".bright_blue(),
      name,
      "API:".bright_blue(),
      api_name,
      "Is unary:".bright_blue(),
      is_unary
    );
    println!("Allow? [y/n]");

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_ok() {
      match input.trim().to_lowercase().as_str() {
        "y" | "yes" => PromptResponse::Allow,
        _ => PromptResponse::Deny,
      }
    } else {
      println!("Failed to read input, denying permission");
      PromptResponse::Deny
    }
  }
}

// Custom module loader that handles npm: scheme
struct NpmSchemeModuleLoader {
  npm_loader: Arc<npm_module_loader::NpmModuleLoader>,
  node_imports_resolver: Arc<node_imports_resolver::NodeImportsResolver>,
}

impl NpmSchemeModuleLoader {
  fn new() -> Result<Self, AnyError> {
    let cache_dir = dirs::cache_dir()
      .ok_or_else(|| deno_error::JsErrorBox::generic("Failed to get cache directory"))?
      .join("deno")
      .join("npm");

    Ok(Self {
      npm_loader: Arc::new(npm_module_loader::NpmModuleLoader::new()?),
      node_imports_resolver: Arc::new(node_imports_resolver::NodeImportsResolver::new(cache_dir)),
    })
  }

  /// Check if this is a bare import (not relative, not absolute, not npm:, not #)
  fn is_bare_import(&self, specifier: &str) -> bool {
    !specifier.starts_with('.')
      && !specifier.starts_with('/')
      && !specifier.starts_with("npm:")
      && !specifier.starts_with('#')
      && !specifier.contains("://")
      && !specifier.is_empty()
  }

  /// Apply Node.js-style module resolution for npm packages
  fn try_node_resolution(&self, base_url: &ModuleSpecifier) -> Result<ModuleSpecifier, AnyError> {
    if let Ok(path) = base_url.to_file_path() {
      // Try different extensions
      for ext in &[".js", ".json", ".ts", ".mjs", ".cjs"] {
        let path_with_ext = path.with_extension(&ext[1 ..]); // Remove the dot
        if path_with_ext.exists() {
          return Ok(
            ModuleSpecifier::from_file_path(path_with_ext)
              .map_err(|_| anyhow::anyhow!("Failed to create file URL"))?,
          );
        }
      }

      // Try index files in directory
      if path.is_dir() {
        for index_file in &["index.js", "index.json", "index.ts", "index.mjs"] {
          let index_path = path.join(index_file);
          if index_path.exists() {
            return Ok(
              ModuleSpecifier::from_file_path(index_path)
                .map_err(|_| anyhow::anyhow!("Failed to create file URL"))?,
            );
          }
        }
      }

      // Try treating as directory with index
      let dir_path = path
        .parent()
        .unwrap_or(&path)
        .join(path.file_name().unwrap_or_default());
      for index_file in &["index.js", "index.json", "index.ts", "index.mjs"] {
        let index_path = dir_path.join(index_file);
        if index_path.exists() {
          return Ok(
            ModuleSpecifier::from_file_path(index_path)
              .map_err(|_| anyhow::anyhow!("Failed to create file URL"))?,
          );
        }
      }
    }

    Err(anyhow::anyhow!("No resolution found"))
  }
}

impl ModuleLoader for NpmSchemeModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, deno_error::JsErrorBox> {
    println!("📦 Resolving: {} from {}", specifier, referrer);

    // Handle package imports starting with #
    if specifier.starts_with('#') {
      println!("  🔍 Package import detected: {}", specifier);
      let referrer_spec = ModuleSpecifier::parse(referrer)
        .map_err(|e| deno_error::JsErrorBox::generic(format!("Invalid referrer: {}", e)))?;

      match self
        .node_imports_resolver
        .resolve_package_import(specifier, &referrer_spec)
      {
        Ok(resolved) => {
          println!("  ✅ Package import resolved to: {}", resolved);
          return Ok(resolved);
        }
        Err(e) => {
          println!("  ❌ Package import resolution failed: {}", e);
          return Err(e);
        }
      }
    }

    // Handle npm: scheme specifiers
    if let Ok(npm_ref) = NpmPackageReqReference::from_str(specifier) {
      println!("  ✅ npm: scheme detected: {}", specifier);

      // Try to find if the package is already downloaded in cache
      let package_name = &npm_ref.req().name;
      let cache_dir = dirs::cache_dir()
        .ok_or_else(|| deno_error::JsErrorBox::generic("Failed to get cache directory"))?
        .join("deno")
        .join("npm")
        .join("registry.npmjs.org")
        .join(package_name);

      // Look for any version directory (we'll take the first one found)
      if cache_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
          for entry in entries.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
              let version_dir = entry.path();

              // Try to find the main file
              if let Ok(package_json_path) = version_dir.join("package.json").canonicalize() {
                if let Ok(package_json_content) = std::fs::read_to_string(&package_json_path) {
                  if let Ok(package_json) =
                    serde_json::from_str::<serde_json::Value>(&package_json_content)
                  {
                    let main_field = package_json
                      .get("main")
                      .and_then(|v| v.as_str())
                      .unwrap_or("index.js");

                    let main_file_path = version_dir.join(main_field);
                    if main_file_path.exists() {
                      println!("  🎯 Found cached package at: {}", main_file_path.display());
                      return ModuleSpecifier::from_file_path(main_file_path).map_err(|_| {
                        deno_error::JsErrorBox::generic(
                          "Failed to create file URL for cached package",
                        )
                      });
                    }
                  }
                }
              }
            }
          }
        }
      }

      // If not found in cache, return npm: URL for the loader to handle
      println!("  ⏳ Package not in cache, will download during load");
      return ModuleSpecifier::parse(specifier).map_err(|e| {
        deno_error::JsErrorBox::generic(format!("Failed to parse npm specifier: {}", e))
      });
    } else if self.is_bare_import(specifier) {
      // Handle bare imports differently based on where they come from
      println!("  🔍 Bare import detected: {}", specifier);

      // If this is a bare import from within an npm package, try to resolve it as an npm package
      if referrer.contains("/npm/registry.npmjs.org/") {
        println!("  📦 Bare import from within npm package, trying to resolve as npm dependency");

        // For now, convert to npm: specifier
        // In a full implementation, this would resolve using the package's node_modules
        let npm_specifier = format!("npm:{}", specifier);
        println!("  🔄 Converting to npm specifier: {}", npm_specifier);

        // Try to find if the package is already downloaded
        let cache_dir = dirs::cache_dir()
          .ok_or_else(|| deno_error::JsErrorBox::generic("Failed to get cache directory"))?
          .join("deno")
          .join("npm")
          .join("registry.npmjs.org")
          .join(specifier);

        if cache_dir.exists() {
          // Look for the first version directory
          if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
              if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                let version_dir = entry.path();

                // Try to find the main file
                if let Ok(package_json_path) = version_dir.join("package.json").canonicalize() {
                  if let Ok(package_json_content) = std::fs::read_to_string(&package_json_path) {
                    if let Ok(package_json) =
                      serde_json::from_str::<serde_json::Value>(&package_json_content)
                    {
                      let main_field = package_json
                        .get("main")
                        .and_then(|v| v.as_str())
                        .unwrap_or("index.js");

                      let main_file_path = version_dir.join(main_field);
                      if main_file_path.exists() {
                        println!(
                          "  🎯 Found cached npm dependency at: {}",
                          main_file_path.display()
                        );
                        return ModuleSpecifier::from_file_path(main_file_path).map_err(|_| {
                          deno_error::JsErrorBox::generic(
                            "Failed to create file URL for cached package",
                          )
                        });
                      }
                    }
                  }
                }
              }
            }
          }
        }

        // If not found, return npm: URL for loader to handle
        return ModuleSpecifier::parse(&npm_specifier).map_err(|e| {
          deno_error::JsErrorBox::generic(format!("Failed to parse npm specifier: {}", e))
        });
      } else {
        // For bare imports from user code, convert to npm:
        let npm_specifier = format!("npm:{}", specifier);
        println!("  🔄 Converting to npm specifier: {}", npm_specifier);

        return ModuleSpecifier::parse(&npm_specifier).map_err(|e| {
          deno_error::JsErrorBox::generic(format!("Failed to parse converted npm specifier: {}", e))
        });
      }
    } else {
      // Regular module resolution
      let resolved =
        resolve_import(specifier, referrer).map_err(|e| deno_error::JsErrorBox::from_err(e))?;

      // If this is a file URL in an npm package, apply Node.js resolution rules
      if resolved.scheme() == "file" && referrer.contains("/npm/registry.npmjs.org/") {
        if let Ok(resolved_with_extension) = self.try_node_resolution(&resolved) {
          println!(
            "  📝 Applied Node.js resolution: {} -> {}",
            resolved, resolved_with_extension
          );
          return Ok(resolved_with_extension);
        }
      }

      Ok(resolved)
    }
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    _options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    println!("📥 Loading: {}", module_specifier);

    // Handle npm: URLs
    if module_specifier.scheme() == "npm" {
      if let Ok(npm_ref) = NpmPackageReqReference::from_specifier(module_specifier) {
        println!("\n🔍 npm package load requested:");
        println!("  📦 Package: {}", npm_ref.req().name);
        println!("  📦 Version: {}", npm_ref.req().version_req);

        // Try to load the npm module
        let npm_loader = self.npm_loader.clone();
        let npm_ref_clone = npm_ref.clone();

        return ModuleLoadResponse::Async(Box::pin(async move {
          match npm_loader.load_npm_module(&npm_ref_clone).await {
            Ok(module_source) => Ok(module_source),
            Err(e) => Err(e),
          }
        }));
      }
    }

    // For non-npm modules, try to load from file system
    if module_specifier.scheme() == "file" {
      let path = module_specifier.to_file_path().unwrap();
      match std::fs::read_to_string(&path) {
        Ok(source_code) => {
          // Determine media type from extension
          let media_type = MediaType::from_specifier(module_specifier);

          let (code, module_type) =
            match media_type {
              MediaType::TypeScript | MediaType::Tsx | MediaType::Mts | MediaType::Cts => {
                println!("🔄 Transpiling TypeScript file: {}", path.display());

                // Parse and transpile TypeScript to JavaScript
                let parsed = match deno_ast::parse_module(ParseParams {
                  specifier: module_specifier.clone(),
                  text: source_code.clone().into(),
                  media_type,
                  capture_tokens: false,
                  scope_analysis: false,
                  maybe_syntax: None,
                }) {
                  Ok(parsed) => parsed,
                  Err(e) => {
                    return ModuleLoadResponse::Sync(Err(deno_error::JsErrorBox::generic(
                      format!("Failed to parse TypeScript: {}", e),
                    )));
                  }
                };

                match parsed.transpile(
                  &TranspileOptions::default(),
                  &TranspileModuleOptions::default(),
                  &EmitOptions {
                    source_map: SourceMapOption::None,
                    ..Default::default()
                  },
                ) {
                  Ok(transpiled) => {
                    println!("  ✅ Successfully transpiled!");
                    let transpiled_source = transpiled.into_source();

                    // Debug: Check if npm imports are preserved
                    if transpiled_source.text.contains("npm:") {
                      println!("  📦 Transpiled code still contains npm: imports");
                    }

                    (transpiled_source.text, ModuleType::JavaScript)
                  }
                  Err(e) => {
                    return ModuleLoadResponse::Sync(Err(deno_error::JsErrorBox::generic(
                      format!("Failed to transpile TypeScript: {}", e),
                    )));
                  }
                }
              }
              MediaType::Json => (source_code, ModuleType::Json),
              _ => (source_code, ModuleType::JavaScript),
            };

          ModuleLoadResponse::Sync(Ok(ModuleSource::new(
            module_type,
            ModuleSourceCode::String(code.into()),
            module_specifier,
            None,
          )))
        }
        Err(e) => ModuleLoadResponse::Sync(Err(deno_error::JsErrorBox::generic(format!(
          "Failed to load file: {}",
          e
        )))),
      }
    } else {
      ModuleLoadResponse::Sync(Err(deno_error::JsErrorBox::generic(format!(
        "Unsupported scheme: {}",
        module_specifier.scheme()
      ))))
    }
  }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), AnyError> {
  let js_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("./test-files/dayjs_test.ts");
  let main_module = ModuleSpecifier::from_file_path(js_path).unwrap();

  let fs = Arc::new(RealFs);
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(
    sys_traits::impls::RealSys,
  ));
  let permission_container =
    PermissionsContainer::new(permission_desc_parser, Permissions::none_with_prompt());

  set_prompter(Box::new(CustomPrompter));

  let snapshot_options = SnapshotOptions::default();

  // Create the necessary components for npm resolution - keep them simple for now
  let _sys = RealSys;
  let _node_sys = NodeResolutionSys::new(RealSys, None);
  let _pkg_json_resolver = Arc::new(PackageJsonResolver::new(RealSys, None));

  // Create our custom module loader
  let module_loader = Rc::new(NpmSchemeModuleLoader::new()?);

  let mut worker = MainWorker::bootstrap_from_options::<
    DenoInNpmPackageChecker,
    ByonmNpmResolver<RealSys>,
    RealSys,
  >(
    &main_module,
    WorkerServiceOptions {
      module_loader,
      permissions: permission_container,
      blob_store: Default::default(),
      broadcast_channel: Default::default(),
      feature_checker: Default::default(),
      node_services: None,
      npm_process_state_provider: Default::default(),
      root_cert_store_provider: Default::default(),
      shared_array_buffer_store: Default::default(),
      compiled_wasm_module_store: Default::default(),
      v8_code_cache: Default::default(),
      fs,
      deno_rt_native_addon_loader: Default::default(),
      fetch_dns_resolver: Default::default(),
      bundle_provider: None,
    },
    WorkerOptions {
      extensions: vec![
        snapshot_options_extension::init(snapshot_options),
        example_extension::init(),
      ],
      ..Default::default()
    },
  );

  println!("🚀 Starting worker with npm: scheme support...");
  println!("📦 This example demonstrates npm: import recognition");
  println!("📝 The TypeScript file imports several npm packages using npm: scheme");
  println!("📚 See the error messages for implementation guidance");
  println!();

  worker.execute_main_module(&main_module).await?;
  worker.run_event_loop(false).await?;

  println!("\nExit code: {}", worker.exit_code());

  Ok(())
}

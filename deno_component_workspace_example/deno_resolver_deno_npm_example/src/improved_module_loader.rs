use std::rc::Rc;
use std::sync::Arc;

use deno_core::{
    ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier,
    ModuleType, RequestedModuleType, ResolutionKind, resolve_import,
    error::AnyError,
};
use deno_ast::{
    EmitOptions, MediaType, ParseParams, SourceMapOption, TranspileModuleOptions, TranspileOptions,
};
use deno_error::JsErrorBox;
use deno_semver::npm::NpmPackageReqReference;

use crate::{npm_module_loader, node_imports_resolver};

/// Enhanced module loader that integrates npm resolution with Node.js-style resolution
pub struct ImprovedModuleLoader {
    npm_loader: Arc<npm_module_loader::NpmModuleLoader>,
    node_imports_resolver: Arc<node_imports_resolver::NodeImportsResolver>,
}

impl ImprovedModuleLoader {
    pub fn new() -> Result<Self, AnyError> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| JsErrorBox::generic("Failed to get cache directory"))?
            .join("deno")
            .join("npm");

        Ok(Self {
            npm_loader: Arc::new(npm_module_loader::NpmModuleLoader::new()?),
            node_imports_resolver: Arc::new(node_imports_resolver::NodeImportsResolver::new(cache_dir)),
        })
    }

    /// Check if this is a bare import (not relative, not absolute, not npm:, not #)
    fn is_bare_import(&self, specifier: &str) -> bool {
        !specifier.starts_with('.') &&
        !specifier.starts_with('/') &&
        !specifier.starts_with("npm:") &&
        !specifier.starts_with('#') &&
        !specifier.contains("://") &&
        !specifier.is_empty()
    }

    /// Resolve import using our enhanced resolution strategy
    fn resolve_enhanced(
        &self,
        specifier: &str,
        referrer: &str,
    ) -> Result<ModuleSpecifier, JsErrorBox> {
        println!("🔍 Enhanced resolving: {} from {}", specifier, referrer);

        // 1. Handle package imports starting with #
        if specifier.starts_with('#') {
            println!("  🔍 Package import detected: {}", specifier);
            let referrer_spec = ModuleSpecifier::parse(referrer)
                .map_err(|e| JsErrorBox::generic(format!("Invalid referrer: {}", e)))?;

            match self.node_imports_resolver.resolve_package_import(specifier, &referrer_spec) {
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

        // 2. Try to parse as npm package reference
        if let Ok(npm_ref) = NpmPackageReqReference::from_str(specifier) {
            println!("  ✅ npm: scheme detected: {}", specifier);
            println!("  📦 Package: {}", npm_ref.req());
            return ModuleSpecifier::parse(specifier)
                .map_err(|e| JsErrorBox::generic(format!("Failed to parse npm specifier: {}", e)));
        }

        // 3. Handle bare imports (convert to npm: scheme)
        if self.is_bare_import(specifier) {
            println!("  🔍 Bare import detected: {}", specifier);
            let npm_specifier = format!("npm:{}", specifier);
            println!("  🔄 Converting to npm specifier: {}", npm_specifier);

            return ModuleSpecifier::parse(&npm_specifier)
                .map_err(|e| JsErrorBox::generic(format!("Failed to parse converted npm specifier: {}", e)));
        }

        // 4. Regular module resolution (relative/absolute paths, URLs)
        resolve_import(specifier, referrer).map_err(|e| JsErrorBox::from_err(e))
    }
}

impl ModuleLoader for ImprovedModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, JsErrorBox> {
        self.resolve_enhanced(specifier, referrer)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
        _requested_module_type: RequestedModuleType,
    ) -> ModuleLoadResponse {
        println!("📥 Enhanced loading: {}", module_specifier);

        // Handle npm: URLs
        if module_specifier.scheme() == "npm" {
            if let Ok(npm_ref) = NpmPackageReqReference::from_specifier(module_specifier) {
                println!("\n🔍 npm package load requested:");
                println!("  📦 Package: {}", npm_ref.req().name);
                println!("  📦 Version: {}", npm_ref.req().version_req);

                // Use our existing npm loader
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

        // Handle regular file: URLs with transpilation
        if module_specifier.scheme() == "file" {
            let path = module_specifier.to_file_path().unwrap();
            match std::fs::read_to_string(&path) {
                Ok(source_code) => {
                    let media_type = MediaType::from_specifier(module_specifier);

                    let (code, module_type) = match media_type {
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
                                    return ModuleLoadResponse::Sync(Err(JsErrorBox::generic(
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
                                    (transpiled_source.text, ModuleType::JavaScript)
                                }
                                Err(e) => {
                                    return ModuleLoadResponse::Sync(Err(JsErrorBox::generic(
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
                Err(e) => ModuleLoadResponse::Sync(Err(JsErrorBox::generic(format!(
                    "Failed to load file: {}",
                    e
                )))),
            }
        } else {
            ModuleLoadResponse::Sync(Err(JsErrorBox::generic(format!(
                "Unsupported scheme: {}",
                module_specifier.scheme()
            ))))
        }
    }
}
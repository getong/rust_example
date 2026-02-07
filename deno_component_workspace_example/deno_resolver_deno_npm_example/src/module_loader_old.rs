use std::{borrow::Cow, cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use deno_ast::{MediaType, ModuleSpecifier, ParseParams, SourceMapOption};
use deno_runtime::{
  deno_core::{
    ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader, ModuleSource,
    ModuleSourceCode, ModuleType, ResolutionKind, error::ModuleLoaderError, resolve_import, url::Url,
  },
  deno_fs::FileSystem,
};
use deno_graph::npm::NpmPackageReqReference;

// Updated to use official deno_graph for npm: specifier parsing

type SourceMapStore = Rc<RefCell<HashMap<String, Vec<u8>>>>;

pub struct TypescriptModuleLoader {
  pub source_maps: SourceMapStore,
}

impl TypescriptModuleLoader {
  pub fn new() -> Self {
    Self {
      source_maps: Rc::new(RefCell::new(HashMap::new())),
    }
  }

  /// Resolve npm: specifier to cached package file: URL
  fn resolve_npm_to_file(&self, npm_specifier: &ModuleSpecifier) -> Result<ModuleSpecifier, ModuleLoaderError> {
    // Parse the npm specifier
    let npm_ref = NpmPackageReqReference::from_specifier(npm_specifier)
      .map_err(|e| ModuleLoaderError::generic(format!("Invalid npm specifier: {}", e)))?;
    
    // Get the base cache directory (similar to Deno's cache structure)
    let cache_dir = dirs::cache_dir()
      .ok_or_else(|| ModuleLoaderError::generic("Could not determine cache directory"))?
      .join("deno_npm_cache")
      .join("packages");
    
    // Build the package cache path: {cache_dir}/packages/{package_name}@{version}/package/
    let package_name = npm_ref.req().name;
    let version_req = &npm_ref.req().version_req;
    
    // For now, use the version requirement as-is (in a real implementation, this would be resolved)
    let package_path = cache_dir.join(format!("{}@{}", package_name, version_req));
    
    // Try to find existing cached package with any version that matches
    let package_dir = if package_path.exists() {
      package_path
    } else {
      // Search for any version of this package in cache
      if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        let mut found_path = None;
        for entry in entries.flatten() {
          if let Some(name) = entry.file_name().to_str() {
            if name.starts_with(&format!("{}@", package_name)) {
              found_path = Some(entry.path());
              break;
            }
          }
        }
        found_path.ok_or_else(|| {
          ModuleLoaderError::generic(format!("Package {} not found in cache", package_name))
        })?
      } else {
        return Err(ModuleLoaderError::generic("Could not read cache directory"));
      }
    };
    
    // Determine the entry point
    let entry_file = if let Some(sub_path) = npm_ref.sub_path() {
      package_dir.join("package").join(sub_path)
    } else {
      // Try package.json main field, then index.js
      let package_json_path = package_dir.join("package").join("package.json");
      if package_json_path.exists() {
        if let Ok(package_json) = std::fs::read_to_string(&package_json_path) {
          if let Ok(json) = serde_json::from_str::<serde_json::Value>(&package_json) {
            if let Some(main) = json.get("main").and_then(|v| v.as_str()) {
              package_dir.join("package").join(main)
            } else {
              package_dir.join("package").join("index.js")
            }
          } else {
            package_dir.join("package").join("index.js")
          }
        } else {
          package_dir.join("package").join("index.js")
        }
      } else {
        package_dir.join("package").join("index.js")
      }
    };
    
    // Convert to file: URL
    let file_url = Url::from_file_path(&entry_file)
      .map_err(|_| ModuleLoaderError::generic("Could not convert path to file URL"))?;
    
    ModuleSpecifier::parse(file_url.as_str())
      .map_err(|e| ModuleLoaderError::generic(format!("Invalid file URL: {}", e)))
  }
}


async fn load_module_with_npm_resolution(
  module_specifier: ModuleSpecifier,
  source_maps: SourceMapStore,
  _fs: Arc<dyn FileSystem>,
  downloader: &Arc<NpmDownloader>,
) -> Result<ModuleSource, ModuleLoaderError> {
  let path = module_specifier
    .to_file_path()
    .map_err(|_| ModuleLoaderError::generic("Only file:// URLs are supported"))?;

  let media_type = MediaType::from_path(&path);
  let (module_type, should_transpile) = match media_type {
    MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => (ModuleType::JavaScript, false),
    MediaType::Jsx => (ModuleType::JavaScript, true),
    MediaType::TypeScript
    | MediaType::Mts
    | MediaType::Cts
    | MediaType::Dts
    | MediaType::Dmts
    | MediaType::Dcts
    | MediaType::Tsx => (ModuleType::JavaScript, true),
    MediaType::Json => (ModuleType::Json, false),
    _ => {
      return Err(ModuleLoaderError::generic(format!(
        "Unknown extension {:?}",
        path.extension()
      )));
    }
  };

  // Read file
  let mut code = std::fs::read_to_string(&path)
    .map_err(|e| ModuleLoaderError::generic(format!("Failed to read file: {}", e)))?;

  // Transform npm: imports to file:// URLs
  code = transform_npm_imports(code, downloader).await?;

  let code = if should_transpile {
    let parsed = deno_ast::parse_module(ParseParams {
      specifier: module_specifier.clone(),
      text: code.into(),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .map_err(|e| ModuleLoaderError::generic(format!("Failed to parse module: {}", e)))?;

    let res = parsed
      .transpile(
        &deno_ast::TranspileOptions {
          imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
          decorators: deno_ast::DecoratorsTranspileOption::Ecma,
          ..Default::default()
        },
        &deno_ast::TranspileModuleOptions::default(),
        &deno_ast::EmitOptions {
          source_map: SourceMapOption::Separate,
          inline_sources: true,
          ..Default::default()
        },
      )
      .map_err(|e| ModuleLoaderError::generic(format!("Failed to transpile: {}", e)))?;

    let res = res.into_source();
    let source_map = res.source_map.unwrap();
    source_maps
      .borrow_mut()
      .insert(module_specifier.to_string(), source_map.into_bytes());
    String::from_utf8(res.text.into_bytes()).unwrap()
  } else {
    code
  };

  Ok(ModuleSource::new(
    module_type,
    ModuleSourceCode::String(code.into()),
    &module_specifier,
    None,
  ))
}


async fn transform_npm_imports(
  mut code: String,
  downloader: &Arc<NpmDownloader>,
) -> Result<String, ModuleLoaderError> {
  let npm_imports = extract_npm_imports(&code);

  for npm_import in npm_imports {
    // Get the cached package (should already be downloaded)
    let package_name = npm_import.strip_prefix("npm:").unwrap_or(&npm_import);
    let (name, version, sub_path) = parse_npm_specifier(package_name);

    // Resolve version if needed
    let resolved_version = if version == "latest" {
      // Would need to resolve latest version, for now use "latest"
      "latest".to_string()
    } else {
      version
    };

    if let Ok(Some(cached_package)) = downloader.cache.get_package(&name, &resolved_version) {
      let file_path = if let Some(sub_path) = sub_path {
        cached_package.path.join("package").join(sub_path)
      } else if let Some(main_path) = downloader.cache.get_main_entry_path(&cached_package) {
        main_path
      } else {
        cached_package.path.join("package").join("index.js")
      };

      if let Ok(file_url) = Url::from_file_path(&file_path) {
        let file_url_str = file_url.to_string();

        // Replace npm: import with file:// URL
        code = code.replace(
          &format!("\"{}\"", npm_import),
          &format!("\"{}\"", file_url_str),
        );
        code = code.replace(&format!("'{}'", npm_import), &format!("'{}'", file_url_str));
      }
    }
  }

  Ok(code)
}

fn load(
  source_maps: SourceMapStore,
  module_specifier: &ModuleSpecifier,
) -> Result<ModuleSource, ModuleLoaderError> {
  println!("👀 load: {}", module_specifier);

  let (code, should_transpile, media_type, module_type) = if module_specifier.scheme() == "file" {
    let path = module_specifier.to_file_path().map_err(|_| {
      deno_error::JsErrorBox::generic(
        "There was an error converting the module specifier to a file path",
      )
    })?;

    let media_type = MediaType::from_path(&path);
    let (module_type, should_transpile) = match MediaType::from_path(&path) {
      MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => (ModuleType::JavaScript, false),
      MediaType::Jsx => (ModuleType::JavaScript, true),
      MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
      | MediaType::Tsx => (ModuleType::JavaScript, true),
      MediaType::Json => (ModuleType::Json, false),
      _ => {
        return Err(deno_error::JsErrorBox::generic(format!(
          "Unknown extension {:?}",
          path.extension()
        )));
      }
    };

    (
      std::fs::read_to_string(&path).map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?,
      should_transpile,
      media_type,
      module_type,
    )
  } else if module_specifier.scheme() == "https" {
    let url = module_specifier.to_string();

    // Try to use system proxy settings if available
    let mut builder = ureq::Agent::config_builder();

    // Check for proxy environment variables
    if let Ok(https_proxy) = std::env::var("https_proxy") {
      eprintln!("Using https_proxy: {}", https_proxy);
      if let Ok(proxy_url) = ureq::Proxy::new(&https_proxy) {
        builder = builder.proxy(Some(proxy_url));
      }
    } else if let Ok(http_proxy) = std::env::var("http_proxy") {
      eprintln!("Using http_proxy: {}", http_proxy);
      if let Ok(proxy_url) = ureq::Proxy::new(&http_proxy) {
        builder = builder.proxy(Some(proxy_url));
      }
    }

    let agent = ureq::Agent::new_with_config(builder.build());

    let response = agent
      .get(&url)
      .header("User-Agent", "Deno")
      .call()
      .map_err(|e| {
        eprintln!("Failed to fetch module from {}: {}", url, e);
        deno_error::JsErrorBox::generic(format!(
          "Failed to fetch module from {}: {}. Check your internet connection and firewall \
           settings.",
          url, e
        ))
      })?;

    let response_text = response.into_body().read_to_string().map_err(|e| {
      eprintln!("Failed to read response body from {}: {:?}", url, e);
      deno_error::JsErrorBox::generic(format!("Failed to read response body: {}", e))
    })?;

    (
      response_text,
      false,
      MediaType::JavaScript,
      ModuleType::JavaScript,
    )
  } else {
    println!("👀 unknown scheme {:?}", module_specifier.scheme());
    return Err(deno_error::JsErrorBox::generic(format!(
      "Unknown scheme {:?}",
      module_specifier.scheme()
    )));
  };

  let code = if should_transpile {
    let parsed = deno_ast::parse_module(ParseParams {
      specifier: module_specifier.clone(),
      text: code.into(),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?;
    let res = parsed
      .transpile(
        &deno_ast::TranspileOptions {
          imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
          decorators: deno_ast::DecoratorsTranspileOption::Ecma,
          ..Default::default()
        },
        &deno_ast::TranspileModuleOptions::default(),
        &deno_ast::EmitOptions {
          source_map: SourceMapOption::Separate,
          inline_sources: true,
          ..Default::default()
        },
      )
      .map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?;
    let res = res.into_source();
    let source_map = res.source_map.unwrap();
    source_maps
      .borrow_mut()
      .insert(module_specifier.to_string(), source_map.into_bytes());
    String::from_utf8(res.text.into_bytes()).unwrap()
  } else {
    // Check if this is a CommonJS module that needs wrapping
    if is_commonjs_module(&code) {
      println!("WRAP: CommonJS module: {}", module_specifier);
      wrap_commonjs_module(code, module_specifier)
    } else {
      println!("SKIP: Not a CommonJS module: {}", module_specifier);
      code
    }
  };
  Ok(ModuleSource::new(
    module_type,
    ModuleSourceCode::String(code.into()),
    module_specifier,
    None,
  ))
}

impl ModuleLoader for TypescriptModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    // Try to parse as npm package reference using deno_graph
    if specifier.starts_with("npm:") {
      let module_specifier = ModuleSpecifier::parse(specifier).map_err(|e| {
        ModuleLoaderError::generic(format!("Failed to parse npm specifier: {}", e))
      })?;
      
      match NpmPackageReqReference::from_specifier(&module_specifier) {
        Ok(npm_ref) => {
          println!("PARSED NPM REF: {} -> {}", specifier, npm_ref);
          // Return the npm: specifier as-is - it will be handled as external
          Ok(module_specifier)
        }
        Err(e) => {
          Err(ModuleLoaderError::generic(format!("Invalid npm specifier: {}", e)))
        }
      }
    } else {
      // Use standard Deno resolution for non-npm specifiers
      resolve_import(specifier, referrer).map_err(|e| deno_error::JsErrorBox::from_err(e))
    }
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    _options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let source_maps = self.source_maps.clone();

    // Check if this is an npm: specifier
    if module_specifier.scheme() == "npm" {
      // Try to resolve npm: specifier to cached package file
      return match self.resolve_npm_to_file(module_specifier) {
        Ok(file_specifier) => {
          println!("RESOLVED npm: {} -> {}", module_specifier, file_specifier);
          // Load the resolved file: URL
          ModuleLoadResponse::Sync(load(source_maps, &file_specifier))
        }
        Err(e) => ModuleLoadResponse::Sync(Err(e))
      };
    }

    // Regular module loading (sync) for file: URLs and other schemes
    ModuleLoadResponse::Sync(load(source_maps, module_specifier))
  }

  fn get_source_map(&self, specifier: &str) -> Option<Cow<'_, [u8]>> {
    self
      .source_maps
      .borrow()
      .get(specifier)
      .map(|v| Cow::Owned(v.clone()))
  }
}

/// Check if a JavaScript file is using CommonJS patterns
fn is_commonjs_module(code: &str) -> bool {
  // Look for CommonJS patterns
  code.contains("module.exports") ||
  code.contains("exports.") ||
  code.contains("exports[") ||
  // Also check if it doesn't have ES module exports
  (!code.contains("export ") && !code.contains("export{") && !code.contains("export*"))
}

/// Wrap CommonJS module to make it ES module compatible
fn wrap_commonjs_module(code: String, module_specifier: &ModuleSpecifier) -> String {
  // Extract package name from the file path for registration
  let module_name = extract_module_name_from_path(module_specifier);
  
  format!(
    r#"
// CommonJS to ES Module wrapper for: {}

// Register this module's factory function immediately
globalThis.__registerCommonJSModule('{}', function(require, module, exports) {{
{}
}});

// Create execution context
const {{ module, exports, require }} = globalThis.__createCommonJSContext(import.meta.url);

// Execute the module immediately (require calls will be resolved from the registry)
{}

export default module.exports;
export {{ module as __module, exports as __exports }};
"#,
    module_name, module_name, code, code
  )
}


/// Extract module name from npm cache path for CommonJS registration
fn extract_module_name_from_path(module_specifier: &ModuleSpecifier) -> String {
  let path_str = module_specifier.as_str();
  
  // Extract package name from paths like:
  // file:///Users/.../deno_npm_cache/packages/is-even@1.0.0/package/index.js
  if let Some(packages_pos) = path_str.find("/packages/") {
    let after_packages = &path_str[packages_pos + 10..]; // Skip "/packages/"
    if let Some(at_pos) = after_packages.find('@') {
      let package_name = &after_packages[..at_pos];
      return package_name.to_string();
    }
  }
  
  // Fallback to full path
  path_str.to_string()
}

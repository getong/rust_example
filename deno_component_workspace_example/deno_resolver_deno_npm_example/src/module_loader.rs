use std::{borrow::Cow, cell::RefCell, collections::HashMap, rc::Rc};

use deno_ast::{MediaType, ModuleSpecifier, ParseParams, SourceMapOption};
use deno_runtime::deno_core::{
  ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader, ModuleSource,
  ModuleSourceCode, ModuleType, ResolutionKind, error::ModuleLoaderError, resolve_import, url::Url,
};

// Import our npm specifier parser
use crate::npm_specifier::NpmSpecifier;

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
  fn resolve_npm_to_file(
    &self,
    npm_specifier: &ModuleSpecifier,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    self.resolve_npm_with_deps(npm_specifier, &mut std::collections::HashSet::new())
  }

  /// Resolve npm: specifier and recursively load dependencies
  fn resolve_npm_with_deps(
    &self,
    npm_specifier: &ModuleSpecifier,
    visited: &mut std::collections::HashSet<String>,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    // Parse the npm specifier using our custom parser
    let npm_spec = NpmSpecifier::parse(npm_specifier.as_str())
      .map_err(|e| ModuleLoaderError::generic(format!("Invalid npm specifier: {}", e)))?;

    let package_name = &npm_spec.name;

    // Avoid circular dependencies
    if visited.contains(package_name) {
      return Err(ModuleLoaderError::generic(format!(
        "Circular dependency detected: {}",
        package_name
      )));
    }
    visited.insert(package_name.clone());

    // Get the base cache directory (similar to Deno's cache structure)
    let cache_dir = dirs::cache_dir()
      .ok_or_else(|| ModuleLoaderError::generic("Could not determine cache directory"))?
      .join("deno_npm_cache")
      .join("packages");

    // Build the package cache path: {cache_dir}/packages/{package_name}@{version}/package/
    let version = npm_spec.version.as_deref().unwrap_or("latest");

    // For now, use the version as-is (in a real implementation, this would be resolved)
    let package_path = cache_dir.join(format!("{}@{}", package_name, version));

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
    let entry_file = if let Some(sub_path) = &npm_spec.sub_path {
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

    // Analyze dependencies in the main entry file
    if entry_file.exists() {
      if let Ok(code) = std::fs::read_to_string(&entry_file) {
        let deps = self.extract_commonjs_deps(&code);
        for dep in deps {
          println!("FOUND DEPENDENCY: {} -> {}", package_name, dep);

          // Try to resolve this dependency as an npm package
          if let Ok(dep_specifier) = ModuleSpecifier::parse(&format!("npm:{}@latest", dep)) {
            // Recursively resolve the dependency and load it
            if let Ok(dep_file_spec) = self.resolve_npm_with_deps(&dep_specifier, visited) {
              // Load the dependency to trigger CommonJS registration
              let source_maps = self.source_maps.clone();
              if let Ok(_) = load(source_maps, &dep_file_spec) {
                println!("LOADED DEPENDENCY: {}", dep);
              }
            } else {
              println!("WARNING: Could not resolve dependency: {}", dep);
            }
          }
        }
      }
    }

    // Convert to file: URL
    let file_url = Url::from_file_path(&entry_file)
      .map_err(|_| ModuleLoaderError::generic("Could not convert path to file URL"))?;

    ModuleSpecifier::parse(file_url.as_str())
      .map_err(|e| ModuleLoaderError::generic(format!("Invalid file URL: {}", e)))
  }

  /// Extract CommonJS dependencies from require() calls
  fn extract_commonjs_deps(&self, code: &str) -> Vec<String> {
    let mut deps = Vec::new();

    // Simple regex-like parsing for require() calls
    for line in code.lines() {
      // Look for require('module-name') or require("module-name")
      if let Some(require_pos) = line.find("require(") {
        let after_require = &line[require_pos + 8 ..]; // Skip "require("

        // Find opening quote
        if let Some(quote_start) = after_require.find('"').or_else(|| after_require.find('\'')) {
          let quote_char = after_require.chars().nth(quote_start).unwrap();
          let after_quote = &after_require[quote_start + 1 ..];

          // Find closing quote
          if let Some(quote_end) = after_quote.find(quote_char) {
            let module_name = &after_quote[.. quote_end];

            // Skip relative paths and built-in modules
            if !module_name.starts_with('.')
              && !module_name.starts_with('/')
              && !is_builtin_module(module_name)
            {
              deps.push(module_name.to_string());
            }
          }
        }
      }
    }

    deps
  }
}

/// Check if a module name is a Node.js built-in module
fn is_builtin_module(name: &str) -> bool {
  matches!(
    name,
    "fs"
      | "path"
      | "crypto"
      | "http"
      | "https"
      | "url"
      | "util"
      | "os"
      | "stream"
      | "events"
      | "buffer"
      | "assert"
      | "child_process"
      | "cluster"
      | "dgram"
      | "dns"
      | "domain"
      | "net"
      | "punycode"
      | "querystring"
      | "readline"
      | "repl"
      | "string_decoder"
      | "tls"
      | "tty"
      | "vm"
      | "zlib"
      | "constants"
      | "module"
      | "process"
      | "v8"
  )
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
    // Try to parse as npm package reference using our custom parser
    if specifier.starts_with("npm:") {
      let module_specifier = ModuleSpecifier::parse(specifier)
        .map_err(|e| ModuleLoaderError::generic(format!("Failed to parse npm specifier: {}", e)))?;

      match NpmSpecifier::parse(specifier) {
        Ok(npm_spec) => {
          println!(
            "PARSED NPM SPEC: {} -> name={}, version={:?}, sub_path={:?}",
            specifier, npm_spec.name, npm_spec.version, npm_spec.sub_path
          );
          // Resolve npm: specifier to file: URL immediately during resolution phase
          self.resolve_npm_to_file(&module_specifier)
        }
        Err(e) => Err(ModuleLoaderError::generic(format!(
          "Invalid npm specifier: {}",
          e
        ))),
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

    // npm: specifiers are now resolved to file: URLs in the resolve() method,
    // so we just need to handle regular module loading for file: URLs and other schemes
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

// Register this module's factory function
globalThis.__registerCommonJSModule('{}', function(require, module, exports) {{
{}
}});

// Execution happens through the CommonJS require system
// The module will be executed when first required
const moduleExports = {{
  get default() {{
    // Trigger CommonJS loading through require mechanism
    const {{ require }} = globalThis.__createCommonJSContext(import.meta.url);
    return require('{}');
  }}
}};

export default moduleExports.default;
export {{ moduleExports }};
"#,
    module_name, module_name, code, module_name
  )
}

/// Extract module name from npm cache path for CommonJS registration
fn extract_module_name_from_path(module_specifier: &ModuleSpecifier) -> String {
  let path_str = module_specifier.as_str();

  // Extract package name from paths like:
  // file:///Users/.../deno_npm_cache/packages/is-even@1.0.0/package/index.js
  if let Some(packages_pos) = path_str.find("/packages/") {
    let after_packages = &path_str[packages_pos + 10 ..]; // Skip "/packages/"
    if let Some(at_pos) = after_packages.find('@') {
      let package_name = &after_packages[.. at_pos];
      return package_name.to_string();
    }
  }

  // Fallback to full path
  path_str.to_string()
}

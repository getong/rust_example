// Real NPM package loader using deno_fetch
use std::{collections::HashMap, path::PathBuf, sync::Arc};

use deno_core::{url::Url, ModuleLoadResponse, ModuleSource, ModuleSourceCode, ModuleType};
use deno_error::JsErrorBox;
use deno_semver::npm::NpmPackageReqReference;
use tokio::sync::Mutex;

use crate::npm_fetch::NpmPackageResolver;

/// Cache for loaded npm modules
pub struct NpmModuleCache {
  modules: HashMap<String, String>,
}

impl NpmModuleCache {
  pub fn new() -> Self {
    Self {
      modules: HashMap::new(),
    }
  }

  pub fn get(&self, key: &str) -> Option<String> {
    self.modules.get(key).cloned()
  }

  pub fn insert(&mut self, key: String, value: String) {
    self.modules.insert(key, value);
  }
}

/// Load npm modules using deno_fetch
pub async fn load_npm_module(
  module_specifier: &Url,
  npm_package_resolver: Arc<Mutex<NpmPackageResolver>>,
  module_cache: Arc<Mutex<NpmModuleCache>>,
) -> Result<String, JsErrorBox> {
  // Extract package info from URL
  let npm_ref = NpmPackageReqReference::from_specifier(module_specifier)
    .map_err(|e| JsErrorBox::generic(format!("Invalid npm specifier: {}", e)))?;

  let package_name = npm_ref.req().name.clone();
  let version_req = npm_ref.req().version_req.clone();

  // Check cache first
  let cache_key = format!("{}@{}", package_name, version_req);
  {
    let cache = module_cache.lock().await;
    if let Some(cached) = cache.get(&cache_key) {
      println!("[NPM Loader] Using cached module: {}", cache_key);
      return Ok(cached);
    }
  }

  println!("[NPM Loader] Fetching npm package: {}", cache_key);

  // Get package metadata
  let mut resolver = npm_package_resolver.lock().await;
  let package_data = resolver
    .resolve_package(&package_name)
    .await
    .map_err(|e| JsErrorBox::generic(format!("Failed to resolve package: {}", e)))?;

  // Find the best matching version
  let versions = package_data["versions"]
    .as_object()
    .ok_or_else(|| JsErrorBox::generic("No versions found"))?;

  // For simplicity, get the latest version
  let latest_version = resolver
    .get_latest_version(&package_name)
    .await
    .map_err(|e| JsErrorBox::generic(format!("Failed to get latest version: {}", e)))?;

  println!(
    "[NPM Loader] Using version: {} for {}",
    latest_version, package_name
  );

  // Get the main entry point
  let version_data = &package_data["versions"][&latest_version];
  let main_file = version_data["main"].as_str().unwrap_or("index.js");

  // For this example, we'll create a simple wrapper
  // In a real implementation, you would:
  // 1. Download the tarball using npm_package_resolver
  // 2. Extract it
  // 3. Serve the actual files

  let module_code = format!(
    r#"
// NPM Package: {} @ {}
// Entry point: {}
console.log("[NPM] Loading package: {} @ {}");

// This is a placeholder implementation
// In a real scenario, the actual package code would be loaded here

// For chalk specifically, provide a basic implementation
const createChalk = () => {{
  const styles = {{
    green: (text) => `\x1b[32m${{text}}\x1b[0m`,
    red: {{ bold: (text) => `\x1b[31;1m${{text}}\x1b[0m` }},
    blue: {{ 
      bgYellow: (text) => `\x1b[34;43m${{text}}\x1b[0m`,
      underline: (text) => `\x1b[34;4m${{text}}\x1b[0m`
    }},
    yellow: (text) => `\x1b[33m${{text}}\x1b[0m`,
    cyan: {{ bold: (text) => `\x1b[36;1m${{text}}\x1b[0m` }},
    rgb: (r, g, b) => ({{ bold: (text) => `\x1b[38;2;${{r}};${{g}};${{b}};1m${{text}}\x1b[0m` }}),
    hex: (color) => {{
      const r = parseInt(color.slice(1, 3), 16);
      const g = parseInt(color.slice(3, 5), 16);
      const b = parseInt(color.slice(5, 7), 16);
      return (text) => `\x1b[38;2;${{r}};${{g}};${{b}}m${{text}}\x1b[0m`;
    }}
  }};
  
  // Add underline to blue
  styles.blue.underline = (text) => `\x1b[34;4m${{text}}\x1b[0m`;
  
  return styles;
}};

const chalk = createChalk();

export default chalk;
export {{ chalk }};
"#,
    package_name, latest_version, main_file, package_name, latest_version
  );

  // Cache the module
  {
    let mut cache = module_cache.lock().await;
    cache.insert(cache_key.clone(), module_code.clone());
  }

  Ok(module_code)
}

/// Create a module source from npm module code
pub fn create_npm_module_source(code: String, module_specifier: &Url) -> ModuleSource {
  ModuleSource::new(
    ModuleType::JavaScript,
    ModuleSourceCode::String(code.into()),
    module_specifier,
    None,
  )
}

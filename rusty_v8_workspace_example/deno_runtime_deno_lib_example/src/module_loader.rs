use std::{borrow::Cow, cell::RefCell, collections::HashMap, fs, path::PathBuf, rc::Rc, sync::Arc};

use deno_runtime::{
  deno_core::{
    ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
    RequestedModuleType, ResolutionKind, error::ModuleLoaderError, resolve_import, url::Url,
  },
  deno_fs::FileSystem,
};

// type ModuleLoaderError = deno_runtime::deno_core::error::ModuleLoaderError;

// Store for source maps
type SourceMapStore = Rc<RefCell<HashMap<String, Vec<u8>>>>;

pub struct CustomModuleLoader {
  fs: Arc<dyn FileSystem>,
  source_maps: SourceMapStore,
  npm_cache_dir: PathBuf,
}

impl CustomModuleLoader {
  pub fn new(fs: Arc<dyn FileSystem>) -> Self {
    let npm_cache_dir = std::env::temp_dir().join("deno_npm_cache");
    fs::create_dir_all(&npm_cache_dir).ok();

    Self {
      fs,
      source_maps: Rc::new(RefCell::new(HashMap::new())),
      npm_cache_dir,
    }
  }

  async fn resolve_and_ensure_npm_module(
    &self,
    specifier: &str,
    _referrer: &ModuleSpecifier,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    let package_name = specifier.strip_prefix("npm:").unwrap_or(specifier);
    let (name, version, sub_path) = parse_npm_specifier(package_name);

    // Simple file-based npm resolution
    let package_dir = self.npm_cache_dir.join(&name).join(&version);
    let package_json_path = package_dir.join("package.json");

    // Check if package is already cached
    if !package_json_path.exists() {
      return Err(ModuleLoaderError::generic(format!(
        "npm package '{}@{}' not found in cache. In a full implementation, this would fetch from \
         registry and extract tarball to: {}",
        name,
        version,
        package_dir.display()
      )));
    }

    // Resolve the main entry point or subpath
    let file_path = if let Some(sub_path) = sub_path {
      package_dir.join(sub_path)
    } else {
      // Read package.json to get main entry point
      let package_json = fs::read_to_string(&package_json_path)
        .map_err(|e| ModuleLoaderError::generic(format!("Failed to read package.json: {}", e)))?;

      // Simple JSON parsing for main field (in a real implementation, use serde_json)
      let main_field = if let Some(start) = package_json.find("\"main\"") {
        let after_colon = &package_json[start + 6 ..];
        if let Some(colon_pos) = after_colon.find(':') {
          let value_start = &after_colon[colon_pos + 1 ..];
          if let Some(quote_start) = value_start.find('"') {
            let after_quote = &value_start[quote_start + 1 ..];
            if let Some(quote_end) = after_quote.find('"') {
              Some(after_quote[.. quote_end].to_string())
            } else {
              None
            }
          } else {
            None
          }
        } else {
          None
        }
      } else {
        None
      };

      package_dir.join(main_field.unwrap_or_else(|| "index.js".to_string()))
    };

    // Convert to file URL
    let file_url = Url::from_file_path(&file_path)
      .map_err(|_| ModuleLoaderError::generic("Failed to convert path to URL"))?;

    Ok(ModuleSpecifier::from(file_url))
  }

  fn resolve_npm_module_sync(
    &self,
    specifier: &str,
    _referrer: &ModuleSpecifier,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    let package_name = specifier.strip_prefix("npm:").unwrap_or(specifier);
    let (name, version, sub_path) = parse_npm_specifier(package_name);

    let package_dir = self.npm_cache_dir.join(&name).join(&version);
    let package_json_path = package_dir.join("package.json");

    if !package_json_path.exists() {
      return Err(ModuleLoaderError::generic(format!(
        "npm package '{}@{}' not found in cache at: {}",
        name,
        version,
        package_dir.display()
      )));
    }

    let file_path = if let Some(sub_path) = sub_path {
      package_dir.join(sub_path)
    } else {
      let package_json = fs::read_to_string(&package_json_path)
        .map_err(|e| ModuleLoaderError::generic(format!("Failed to read package.json: {}", e)))?;

      let main_field = if let Some(start) = package_json.find("\"main\"") {
        let after_colon = &package_json[start + 6 ..];
        if let Some(colon_pos) = after_colon.find(':') {
          let value_start = &after_colon[colon_pos + 1 ..];
          if let Some(quote_start) = value_start.find('"') {
            let after_quote = &value_start[quote_start + 1 ..];
            if let Some(quote_end) = after_quote.find('"') {
              Some(after_quote[.. quote_end].to_string())
            } else {
              None
            }
          } else {
            None
          }
        } else {
          None
        }
      } else {
        None
      };

      package_dir.join(main_field.unwrap_or_else(|| "index.js".to_string()))
    };

    let file_url = Url::from_file_path(&file_path)
      .map_err(|_| ModuleLoaderError::generic("Failed to convert path to URL"))?;

    Ok(ModuleSpecifier::from(file_url))
  }
}

pub fn parse_npm_specifier(spec: &str) -> (String, String, Option<String>) {
  // Handle scoped packages like @types/node or @types/node@1.0.0
  if spec.starts_with('@') {
    if let Some(slash_pos) = spec[1 ..].find('/') {
      let scope_and_name_end = slash_pos + 1;
      let after_name = &spec[scope_and_name_end + 1 ..];

      if let Some(at_pos) = after_name.find('@') {
        // @scope/name@version or @scope/name@version/subpath
        let name = spec[.. scope_and_name_end + 1 + at_pos].to_string();
        let rest = &after_name[at_pos + 1 ..];

        if let Some(slash_pos) = rest.find('/') {
          let version = rest[.. slash_pos].to_string();
          let sub_path = rest[slash_pos + 1 ..].to_string();
          (name, version, Some(sub_path))
        } else {
          (name, rest.to_string(), None)
        }
      } else if let Some(slash_pos) = after_name.find('/') {
        // @scope/name/subpath
        let name = spec[.. scope_and_name_end + 1 + slash_pos].to_string();
        let sub_path = after_name[slash_pos + 1 ..].to_string();
        (name, "latest".to_string(), Some(sub_path))
      } else {
        // @scope/name
        (spec.to_string(), "latest".to_string(), None)
      }
    } else {
      // Invalid scoped package
      (spec.to_string(), "latest".to_string(), None)
    }
  } else {
    // Regular packages
    if let Some(at_pos) = spec.find('@') {
      let name = spec[.. at_pos].to_string();
      let rest = &spec[at_pos + 1 ..];

      if let Some(slash_pos) = rest.find('/') {
        let version = rest[.. slash_pos].to_string();
        let sub_path = rest[slash_pos + 1 ..].to_string();
        (name, version, Some(sub_path))
      } else {
        (name, rest.to_string(), None)
      }
    } else if let Some(slash_pos) = spec.find('/') {
      let name = spec[.. slash_pos].to_string();
      let sub_path = spec[slash_pos + 1 ..].to_string();
      (name, "latest".to_string(), Some(sub_path))
    } else {
      (spec.to_string(), "latest".to_string(), None)
    }
  }
}

impl ModuleLoader for CustomModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    if specifier.starts_with("npm:") {
      let referrer_spec = ModuleSpecifier::parse(referrer)
        .map_err(|e| ModuleLoaderError::generic(format!("Invalid referrer URL: {}", e)))?;

      self.resolve_npm_module_sync(specifier, &referrer_spec)
    } else {
      resolve_import(specifier, referrer).map_err(|e| {
        ModuleLoaderError::generic(format!(
          "Failed to resolve import '{}' from '{}': {}",
          specifier, referrer, e
        ))
      })
    }
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleSpecifier>,
    _is_dyn_import: bool,
    _requested_module_type: RequestedModuleType,
  ) -> ModuleLoadResponse {
    let module_specifier = module_specifier.clone();
    let source_maps = self.source_maps.clone();
    let fs = self.fs.clone();

    // Use sync loading like the example
    ModuleLoadResponse::Sync(load_module(module_specifier, source_maps, fs))
  }

  fn get_source_map(&self, specifier: &str) -> Option<Cow<'_, [u8]>> {
    // Return a clone to avoid borrowing issues
    self
      .source_maps
      .borrow()
      .get(specifier)
      .map(|v| Cow::Owned(v.clone()))
  }
}

fn load_module(
  module_specifier: ModuleSpecifier,
  _source_maps: SourceMapStore,
  _fs: Arc<dyn FileSystem>,
) -> Result<ModuleSource, ModuleLoaderError> {
  let path = module_specifier
    .to_file_path()
    .map_err(|_| ModuleLoaderError::generic("Only file:// URLs are supported"))?;

  let media_type = deno_graph::MediaType::from_specifier(&module_specifier);
  let (module_type, should_transpile) = match media_type {
    deno_graph::MediaType::JavaScript | deno_graph::MediaType::Mjs | deno_graph::MediaType::Cjs => {
      (ModuleType::JavaScript, false)
    }
    deno_graph::MediaType::Jsx => (ModuleType::JavaScript, true),
    deno_graph::MediaType::TypeScript
    | deno_graph::MediaType::Mts
    | deno_graph::MediaType::Cts
    | deno_graph::MediaType::Dts
    | deno_graph::MediaType::Dmts
    | deno_graph::MediaType::Dcts
    | deno_graph::MediaType::Tsx => (ModuleType::JavaScript, true),
    deno_graph::MediaType::Json => (ModuleType::Json, false),
    _ => {
      return Err(ModuleLoaderError::generic(format!(
        "Unknown extension {:?}",
        path.extension()
      )));
    }
  };

  // Read file synchronously using std::fs since we're in sync context
  let code = std::fs::read_to_string(&path)
    .map_err(|e| ModuleLoaderError::generic(format!("Failed to read file: {}", e)))?;

  let code = if should_transpile {
    let parsed = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: module_specifier.clone(),
      text: code.into(),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .map_err(|e| ModuleLoaderError::generic(format!("Failed to parse module: {}", e)))?;

    let transpiled = parsed
      .transpile(
        &deno_ast::TranspileOptions::default(),
        &deno_ast::TranspileModuleOptions::default(),
        &deno_ast::EmitOptions::default(),
      )
      .map_err(|e| ModuleLoaderError::generic(format!("Failed to transpile: {}", e)))?;

    // Store source map if available (the API might have changed)
    // For now, just use the transpiled text
    transpiled.into_source().text
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

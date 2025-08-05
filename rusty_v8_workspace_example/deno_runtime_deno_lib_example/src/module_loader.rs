use std::{borrow::Cow, cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use deno_runtime::{
  deno_core::{
    ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
    RequestedModuleType, ResolutionKind, error::ModuleLoaderError, resolve_import,
  },
  deno_fs::FileSystem,
};

// type ModuleLoaderError = deno_runtime::deno_core::error::ModuleLoaderError;

// Store for source maps
type SourceMapStore = Rc<RefCell<HashMap<String, Vec<u8>>>>;

pub struct CustomModuleLoader {
  fs: Arc<dyn FileSystem>,
  source_maps: SourceMapStore,
}

impl CustomModuleLoader {
  pub fn new(fs: Arc<dyn FileSystem>) -> Self {
    Self {
      fs,
      source_maps: Rc::new(RefCell::new(HashMap::new())),
    }
  }
}

// Helper function to demonstrate npm specifier parsing
fn parse_npm_specifier(spec: &str) -> (String, String, Option<String>) {
  // Examples:
  // "hono" -> ("hono", "latest", None)
  // "hono@3.0.0" -> ("hono", "3.0.0", None)
  // "hono@3.0.0/dist/index.js" -> ("hono", "3.0.0", Some("dist/index.js"))

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

impl ModuleLoader for CustomModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    if specifier.starts_with("npm:") {
      // For demonstration, we'll show the structure of what's needed:
      let npm_spec = specifier.strip_prefix("npm:").unwrap();
      let (package_name, version, sub_path) = parse_npm_specifier(npm_spec);

      let error_msg = format!(
        "npm: specifier '{}' parsed as:\n- Package: {}\n- Version: {}\n- Subpath: {}\n\nBased on \
         Deno's worker.rs, npm support requires:\n1. NpmRegistryApi - fetches package metadata \
         from registry.npmjs.org\n2. NpmCache - downloads and caches package tarballs\n3. \
         CliNpmInstaller - manages package installation with caching strategies\n4. \
         CliNpmResolver - resolves npm specifiers to file paths\n5. Integration with module graph \
         for dependency analysis\n\nSee npm_example.rs for the architecture overview.",
        npm_spec,
        package_name,
        version,
        sub_path.as_deref().unwrap_or("(none)")
      );

      return Err(ModuleLoaderError::generic(error_msg));
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
  source_maps: SourceMapStore,
  fs: Arc<dyn FileSystem>,
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

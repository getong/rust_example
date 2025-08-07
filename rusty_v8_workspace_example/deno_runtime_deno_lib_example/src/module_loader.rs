use std::{borrow::Cow, cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use deno_runtime::{
  deno_core::{
    ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
    RequestedModuleType, ResolutionKind, error::ModuleLoaderError, resolve_import, url::Url,
  },
  deno_fs::FileSystem,
};

use crate::{
  npm_downloader::{NpmConfig, NpmDownloader},
  npm_specifier::parse_npm_specifier,
};

// Store for source maps
type SourceMapStore = Rc<RefCell<HashMap<String, Vec<u8>>>>;

pub struct CustomModuleLoader {
  fs: Arc<dyn FileSystem>,
  source_maps: SourceMapStore,
  npm_downloader: Arc<NpmDownloader>,
}

impl CustomModuleLoader {
  pub fn new(fs: Arc<dyn FileSystem>) -> Self {
    let npm_config = NpmConfig::default();
    let npm_downloader =
      Arc::new(NpmDownloader::new(npm_config).expect("Failed to create npm downloader"));

    Self {
      fs,
      source_maps: Rc::new(RefCell::new(HashMap::new())),
      npm_downloader,
    }
  }

  async fn resolve_and_ensure_npm_module(
    &self,
    specifier: &str,
    _referrer: &ModuleSpecifier,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    // Download the package if not already cached
    let cached_package = self
      .npm_downloader
      .download_package(specifier)
      .await
      .map_err(|e| {
        ModuleLoaderError::generic(format!(
          "Failed to download npm package {}: {}",
          specifier, e
        ))
      })?;

    let package_name = specifier.strip_prefix("npm:").unwrap_or(specifier);
    let (_, _, sub_path) = parse_npm_specifier(package_name);

    // Resolve the main entry point or subpath
    let file_path = if let Some(sub_path) = sub_path {
      cached_package.path.join("package").join(sub_path)
    } else {
      // Use the main entry point from cached package or default to index.js
      if let Some(main_path) = self
        .npm_downloader
        .cache
        .get_main_entry_path(&cached_package)
      {
        main_path
      } else {
        cached_package.path.join("package").join("index.js")
      }
    };

    // Convert to file URL
    let file_url = Url::from_file_path(&file_path)
      .map_err(|_| ModuleLoaderError::generic("Failed to convert path to URL"))?;

    Ok(ModuleSpecifier::from(file_url))
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
      // For npm: specifiers, try to resolve synchronously first (if already cached)
      let package_name = specifier.strip_prefix("npm:").unwrap_or(specifier);
      let (name, version, sub_path) = parse_npm_specifier(package_name);

      // Check if package is already cached (try both the specified version and "latest")
      let versions_to_check = if version == "latest" {
        // If looking for "latest", check all cached versions of this package
        if let Ok(packages) = self.npm_downloader.cache.list_packages() {
          let mut found_versions: Vec<String> = packages
            .iter()
            .filter(|p| p.name == name)
            .map(|p| p.version.clone())
            .collect();
          found_versions.sort();
          found_versions.reverse(); // Get the highest version first
          found_versions
        } else {
          vec!["latest".to_string()]
        }
      } else {
        vec![version.clone()]
      };

      for check_version in versions_to_check {
        if let Ok(Some(cached_package)) =
          self.npm_downloader.cache.get_package(&name, &check_version)
        {
          // Package is cached, resolve to actual file path
          let file_path = if let Some(sub_path) = sub_path.clone() {
            cached_package.path.join("package").join(sub_path)
          } else if let Some(main_path) = self
            .npm_downloader
            .cache
            .get_main_entry_path(&cached_package)
          {
            main_path
          } else {
            cached_package.path.join("package").join("index.js")
          };

          let file_url = Url::from_file_path(&file_path).map_err(|_| {
            ModuleLoaderError::generic("Failed to convert cached package path to URL")
          })?;

          return Ok(ModuleSpecifier::from(file_url));
        }
      }

      // Package not cached, return a placeholder URL that we'll resolve in load()
      let npm_url = format!("npm-resolve:{}", specifier);
      let result = ModuleSpecifier::parse(&npm_url).map_err(|e| {
        ModuleLoaderError::generic(format!("Failed to create npm placeholder URL: {}", e))
      })?;
      Ok(result)
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

    // Check if this is an npm-resolve: specifier that needs async resolution
    if module_specifier.scheme() == "npm-resolve" {
      let npm_specifier = module_specifier
        .as_str()
        .strip_prefix("npm-resolve:")
        .unwrap_or("")
        .to_string();
      let downloader = self.npm_downloader.clone();

      return ModuleLoadResponse::Async(Box::pin(async move {
        println!("📥 Downloading npm package: {}", npm_specifier);
        // Download and resolve the npm package
        let cached_package = downloader
          .download_package(&npm_specifier)
          .await
          .map_err(|e| {
            ModuleLoaderError::generic(format!(
              "Failed to download npm package {}: {}",
              npm_specifier, e
            ))
          })?;

        println!(
          "✅ Successfully downloaded and cached: {} v{}",
          cached_package.name, cached_package.version
        );

        let package_name = npm_specifier.strip_prefix("npm:").unwrap_or(&npm_specifier);
        let (_, _, sub_path) = parse_npm_specifier(package_name);

        // Resolve the main entry point or subpath
        let file_path = if let Some(sub_path) = sub_path {
          cached_package.path.join("package").join(sub_path)
        } else {
          // Use the main entry point from cached package or default to index.js
          if let Some(main_path) = downloader.cache.get_main_entry_path(&cached_package) {
            main_path
          } else {
            cached_package.path.join("package").join("index.js")
          }
        };

        // Now load the actual file
        let actual_specifier = Url::from_file_path(&file_path)
          .map_err(|_| ModuleLoaderError::generic("Failed to convert path to URL"))?;
        let actual_module_specifier = ModuleSpecifier::from(actual_specifier);

        load_module(actual_module_specifier, source_maps, fs)
      }));
    }

    // Check if this is a regular file that might contain npm: imports
    if let Ok(path) = module_specifier.to_file_path() {
      if let Ok(content) = std::fs::read_to_string(&path) {
        let npm_imports = extract_npm_imports(&content);

        if !npm_imports.is_empty() {
          // We found npm imports, handle them asynchronously
          let downloader = self.npm_downloader.clone();
          let module_spec_clone = module_specifier.clone();

          return ModuleLoadResponse::Async(Box::pin(async move {
            // First, download all npm dependencies
            for npm_import in npm_imports {
              if let Err(e) = downloader.download_package(&npm_import).await {
                return Err(ModuleLoaderError::generic(format!(
                  "Failed to download npm package {}: {}",
                  npm_import, e
                )));
              }
            }

            // Now load the module with resolved npm imports
            load_module_with_npm_resolution(module_spec_clone, source_maps, fs, &downloader).await
          }));
        }
      }
    }

    // Regular module loading (sync)
    ModuleLoadResponse::Sync(load_module(module_specifier, source_maps, fs))
  }

  fn get_source_map(&self, specifier: &str) -> Option<Cow<'_, [u8]>> {
    self
      .source_maps
      .borrow()
      .get(specifier)
      .map(|v| Cow::Owned(v.clone()))
  }
}

// Helper function to extract npm: imports from module content
fn extract_npm_imports(content: &str) -> Vec<String> {
  let mut imports = Vec::new();

  // Simple regex-like parsing for npm: imports
  for line in content.lines() {
    if let Some(import_start) = line.find("from \"npm:") {
      let after_npm = &line[import_start + 10 ..];
      if let Some(quote_end) = after_npm.find('"') {
        let npm_spec = format!("npm:{}", &after_npm[.. quote_end]);
        imports.push(npm_spec);
      }
    } else if let Some(import_start) = line.find("from 'npm:") {
      let after_npm = &line[import_start + 10 ..];
      if let Some(quote_end) = after_npm.find('\'') {
        let npm_spec = format!("npm:{}", &after_npm[.. quote_end]);
        imports.push(npm_spec);
      }
    }
  }

  imports
}

async fn load_module_with_npm_resolution(
  module_specifier: ModuleSpecifier,
  _source_maps: SourceMapStore,
  _fs: Arc<dyn FileSystem>,
  downloader: &Arc<NpmDownloader>,
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

  // Read file
  let mut code = std::fs::read_to_string(&path)
    .map_err(|e| ModuleLoaderError::generic(format!("Failed to read file: {}", e)))?;

  // Transform npm: imports to file:// URLs
  code = transform_npm_imports(code, downloader).await?;

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
  let mut code = std::fs::read_to_string(&path)
    .map_err(|e| ModuleLoaderError::generic(format!("Failed to read file: {}", e)))?;

  // Special handling for stream-chat package
  if path.to_string_lossy().contains("stream-chat") && path.to_string_lossy().contains("index.js") {
    // Replace import_https.default.Agent with a constructor function
    code = code.replace(
      "new import_https.default.Agent({",
      "new (function HttpsAgent(options = {}) { this.options = options || {}; this.protocol = \
       'https:'; this.maxSockets = options.maxSockets || Infinity; this.maxFreeSockets = \
       options.maxFreeSockets || 256; this.maxCachedSessions = options.maxCachedSessions || 100; \
       this.keepAlive = options.keepAlive || false; this.keepAliveMsecs = options.keepAliveMsecs \
       || 1000; })({",
    );

    // Replace any usage of import_jsonwebtoken.default.sign
    code = code.replace(
      "import_jsonwebtoken.default.sign(",
      "(async (payload, secret, options = {}) => { const header = { alg: 'HS256', typ: 'JWT' }; \
       const encodedHeader = btoa(JSON.stringify(header)).replace(/=/g, ''); const now = \
       Math.floor(Date.now() / 1000); const finalPayload = { ...payload, iat: options.noTimestamp \
       ? undefined : now, exp: options.expiresIn ? now + options.expiresIn : undefined }; \
       Object.keys(finalPayload).forEach(key => { if (finalPayload[key] === undefined) { delete \
       finalPayload[key]; } }); const encodedPayload = \
       btoa(JSON.stringify(finalPayload)).replace(/=/g, ''); const token = \
       `${encodedHeader}.${encodedPayload}`; const key = await \
       globalThis.crypto.subtle.importKey('raw', new TextEncoder().encode(secret), { name: \
       'HMAC', hash: 'SHA-256' }, false, ['sign']); const signature = await \
       globalThis.crypto.subtle.sign('HMAC', key, new TextEncoder().encode(token)); const \
       encodedSignature = btoa(String.fromCharCode(...new Uint8Array(signature))).replace(/\\+/g, \
       '-').replace(/\\//g, '_').replace(/=/g, ''); return `${token}.${encodedSignature}`; })(",
    );

    // Fix the createToken method to use JWTServerToken instead of JWTUserToken when server-side
    code = code.replace(
      "return JWTUserToken(this.secret, userID, extra, {});",
      "return JWTServerToken(this.secret, userID, extra);",
    );

    println!("🔧 Patched stream-chat module for HTTPS Agent and JWT compatibility");
  }

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

    transpiled.into_source().text
  } else {
    // Check if this is a CommonJS module that needs wrapping
    if is_commonjs_module(&code) {
      wrap_commonjs_module(code)
    } else {
      code
    }
  };

  Ok(ModuleSource::new(
    module_type,
    ModuleSourceCode::String(code.into()),
    &module_specifier,
    None,
  ))
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
fn wrap_commonjs_module(code: String) -> String {
  format!(
    r#"
// CommonJS to ES Module wrapper
const {{ module, exports }} = globalThis.__createCommonJSContext();
{}
export default module.exports;
export {{ module as __module, exports as __exports }};
"#,
    code
  )
}
